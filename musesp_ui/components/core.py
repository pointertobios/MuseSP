"""核心组件基类和布局枚举。"""

from collections.abc import Callable
from enum import Enum, auto

import pygame

from musesp_config.config import config


class Constraintable(Enum):
    NONE = auto()
    MINIMUM = auto()
    MAXIMUM = auto()


class Direction(Enum):
    VERTICAL = auto()
    HORIZONTAL = auto()


class Component:
    """UI 组件基类，提供布局、事件、绘制的完整框架。

    ========== 布局系统 ==========

    每个组件通过 ``h_constraint`` / ``v_constraint`` 声明其在父组件中的
    布局行为，由父组件的 ``layout()`` 在父组件尺寸确定后统一计算。

    约束类型（:class:`Constraintable`）：
        - ``NONE``      — 不参与布局，保持原始位置和尺寸
        - ``MINIMUM``   — 分配 ``min_width`` / ``min_height``，
                          显式 min>0 受保护不被压缩，min==0 可被 MAXIMUM 挤占至 0
        - ``MAXIMUM``   — 均分父组件剩余空间，不受 MINIMUM 挤压；
                          可设 ``max_width`` / ``max_height`` 限制上限（0=无限制）；
                          多个 MAXIMUM 通过迭代算法均分：触及上限的组件锁定，
                          其余继续扩大直到空间用尽

    布局方向（:class:`Direction`）：
        - ``VERTICAL``   — 主轴为 y，子组件从上到下排列，交叉轴填满宽度
        - ``HORIZONTAL`` — 主轴为 x，子组件从左到右排列，交叉轴填满高度

    居中（主轴方向）：
        - ``centered_vertical``   — 仅 VERTICAL 布局时生效，子组件组垂直居中
        - ``centered_horizontal`` — 仅 HORIZONTAL 布局时生效，子组件组水平居中

    尺寸属性：
        - ``min_width`` / ``min_height`` — MINIMUM 或 MAXIMUM 的保底尺寸
        - ``max_width`` / ``max_height`` — MAXIMUM 的上限（0=无限制）
        - ``width`` / ``height``         — 实际尺寸，由 ``layout()`` 计算

    ``layout()`` 递归调用，父组件先计算自身子组件布局，再触发子组件对孙组件的布局。

    ========== 事件系统 ==========

    ``dispatch_event(event) -> bool`` 将 pygame 事件从根组件向下传递：

        1. 事件坐标平移到组件局部坐标系
        2. 调用 ``_handle_event()`` 触发内置行为并调用用户 handler
        3. 若未停止，按子组件顺序递归传递
        4. 返回 ``True`` 继续传播，``False`` 停止

    内置事件处理：
        - ``MOUSEMOTION``     — 检测 hover，触发 mouse_enter / mouse_exit
        - ``MOUSEBUTTONDOWN`` — 检测按下，设置 ``_pressed``，触发 mouse_down
        - ``MOUSEBUTTONUP``   — 清除 ``_pressed``，触发 mouse_up；
                                若按下+释放在组件内则触发 mouse_click
        - ``KEYDOWN``         — 记录按键到 ``_pressed_keys``，触发 key_down
        - ``KEYUP``           — 移除按键，触发 key_up

    Handler 必须返回 ``bool``：
        - ``True``  — 继续向子组件传播
        - ``False`` — 停止传播，后续子组件不再收到此事件

    同一事件可绑定多个 handler，全部执行，任一返回 ``False`` 即停止传播。

    ``force_mouse_exit()`` 递归清除整棵树的 hover/pressed 状态，
    页面切换时由 Router 调用，避免卡状态。

    绑定方法：
        - ``bind_on_mouse_enter / exit / down / up / click / hold``
        - ``bind_on_key_down / up / click / hold``

    ========== 绘制系统 ==========

    子类重写 ``_draw_self(surface, draw_x, draw_y)`` 绘制自身内容，
    坐标系为父组件传递的绝对画布坐标。

    - ``draw(surface)``          — 入口，递归绘制整棵组件树
    - ``draw_debug(surface)``    — 绘制调试边框（受配置 ``debug.ui.component_border`` 控制）

    ========== 组件树 ==========

    - ``add_sub_component(comp)`` — 添加子组件
    - ``sub_components``          — 只读属性，返回子组件列表
    """

    def __init__(self, x: int = 0, y: int = 0, width: int = 0, height: int = 0):
        self.x = x
        self.y = y
        self.width = width
        self.height = height
        self.min_width = 0
        self.min_height = 0
        self.max_width = 0
        self.max_height = 0
        self.h_constraint = Constraintable.NONE
        self.v_constraint = Constraintable.NONE
        self.layout_direction = Direction.VERTICAL
        self.centered_horizontal = True
        self.centered_vertical = True
        self._hovered = False
        self._pressed = False
        self._pressed_keys: set[int] = set()
        self._sub_components: list[Component] = []
        self._handlers: dict[str, list[Callable]] = {k: [] for k in [
            "mouse_enter", "mouse_exit", "mouse_down", "mouse_up",
            "mouse_click", "mouse_hold",
            "key_down", "key_up", "key_click", "key_hold",
        ]}

    @property
    def sub_components(self) -> list["Component"]:
        """子组件列表（只读）。"""
        return self._sub_components

    def add_sub_component(self, component: "Component") -> None:
        """添加子组件到组件树末尾。"""
        self._sub_components.append(component)

    def layout(self) -> None:
        """递归计算整棵子树的布局。

        先根据 ``layout_direction`` 沿主轴排列子组件，
        再递归调用每个子组件的 ``layout()``。"""
        if self.layout_direction == Direction.VERTICAL:
            self._layout_axis("v", "y", "height", "min_height",
                              "v_constraint", "width")
        else:
            self._layout_axis("h", "x", "width", "min_width",
                              "h_constraint", "height")
        for child in self._sub_components:
            child.layout()

    def _layout_axis(self, axis: str, pos_attr: str, size_attr: str,
                     min_attr: str, constraint_attr: str, cross_attr: str) -> None:
        parent_size = getattr(self, size_attr)
        constrained = [c for c in self._sub_components
                       if getattr(c, constraint_attr) != Constraintable.NONE]
        if not constrained:
            return

        max_attr = "max_" + size_attr
        cross = getattr(self, cross_attr)

        maximums: list[Component] = []
        minimum_fixed: list[Component] = []
        minimum_zero: list[Component] = []
        for c in constrained:
            if getattr(c, constraint_attr) == Constraintable.MAXIMUM:
                maximums.append(c)
            elif getattr(c, min_attr) > 0:
                minimum_fixed.append(c)
            else:
                minimum_zero.append(c)

        total_fixed = sum(getattr(c, min_attr) for c in minimum_fixed)

        sizes: dict[Component, int] = {}
        for c in minimum_fixed:
            sizes[c] = getattr(c, min_attr)
        for c in minimum_zero:
            sizes[c] = 0

        if maximums:
            uncapped = list(maximums)
            while True:
                capped_total = sum(sizes.get(c, 0)
                                   for c in maximums if c not in uncapped)
                uncapped_min = sum(getattr(c, min_attr) for c in uncapped)
                remaining = parent_size - total_fixed - capped_total - uncapped_min
                share = max(0, remaining // max(len(uncapped), 1))

                still_uncapped: list[Component] = []
                capped_this_round = False
                for c in uncapped:
                    size = getattr(c, min_attr) + share
                    max_s = getattr(c, max_attr, 0)
                    if max_s > 0 and size > max_s:
                        sizes[c] = max_s
                        capped_this_round = True
                    else:
                        still_uncapped.append(c)
                if not capped_this_round:
                    for c in uncapped:
                        sizes[c] = getattr(c, min_attr) + share
                    break
                uncapped = still_uncapped
        else:
            remaining = parent_size - total_fixed
            share = max(0, remaining // max(len(minimum_zero), 1))
            for c in minimum_zero:
                size = share
                max_s = getattr(c, max_attr, 0)
                if max_s > 0 and size > max_s:
                    size = max_s
                sizes[c] = size

        pos = 0
        for child in self._sub_components:
            constraint = getattr(child, constraint_attr)
            if constraint == Constraintable.NONE:
                continue
            setattr(child, pos_attr, pos)
            setattr(child, cross_attr, cross)
            setattr(child, size_attr, sizes[child])
            pos += sizes[child]

        remaining = parent_size - pos
        if remaining > 0:
            if axis == "v" and self.centered_vertical:
                offset = remaining // 2
                for child in self._sub_components:
                    if getattr(child, constraint_attr) != Constraintable.NONE:
                        setattr(child, pos_attr, getattr(
                            child, pos_attr) + offset)
            elif axis == "h" and self.centered_horizontal:
                offset = remaining // 2
                for child in self._sub_components:
                    if getattr(child, constraint_attr) != Constraintable.NONE:
                        setattr(child, pos_attr, getattr(
                            child, pos_attr) + offset)

    def _bind(self, event: str, handler: Callable) -> None:
        self._handlers[event].append(handler)

    def _in_rect(self, pos: tuple[int, int]) -> bool:
        x, y = pos
        return 0 <= x <= self.width and 0 <= y <= self.height

    def _emit(self, name: str, event: pygame.event.Event) -> bool:
        propagate = True
        for handler in self._handlers[name]:
            result = handler(event)
            if not isinstance(result, bool):
                raise TypeError(
                    f"事件 handler [{name}] 必须返回 bool，实际返回 {type(result).__name__}")
            if not result:
                propagate = False
        return propagate

    def _shift_event(self, event: pygame.event.Event, dx: int, dy: int) -> pygame.event.Event:
        if event.type in (pygame.MOUSEMOTION, pygame.MOUSEBUTTONDOWN, pygame.MOUSEBUTTONUP):
            attrs = dict(event.dict)
            ex, ey = event.pos
            attrs["pos"] = (ex + dx, ey + dy)
            return pygame.event.Event(event.type, attrs)
        return event

    def draw(self, surface: pygame.Surface) -> None:
        """递归绘制整棵组件树到目标 surface。"""
        self._draw_internal(surface, 0, 0)

    def draw_debug(self, surface: pygame.Surface) -> None:
        """绘制调试边框（绿色 2px），受 ``debug.ui.component_border`` 配置控制。"""
        if not config.debug.ui.component_border:
            return
        self._draw_debug_internal(surface, 0, 0)

    def _draw_debug_internal(self, surface: pygame.Surface, offset_x: int, offset_y: int) -> None:
        draw_x = self.x + offset_x
        draw_y = self.y + offset_y
        pygame.draw.rect(surface, (0, 255, 0),
                         pygame.Rect(draw_x, draw_y, self.width, self.height), 2)
        for child in self._sub_components:
            child._draw_debug_internal(surface, draw_x, draw_y)

    def _draw_internal(self, surface: pygame.Surface, offset_x: int, offset_y: int) -> None:
        draw_x = self.x + offset_x
        draw_y = self.y + offset_y
        self._draw_self(surface, draw_x, draw_y)
        for child in self._sub_components:
            child._draw_internal(surface, draw_x, draw_y)

    def _draw_self(self, surface: pygame.Surface, draw_x: int, draw_y: int) -> None:
        pass

    def dispatch_event(self, event: pygame.event.Event) -> bool:
        """向组件树分发 pygame 事件。

        坐标系平移到组件局部空间后依次处理自身和子组件，
        返回 ``True`` 继续传播，``False`` 停止。
        """
        local = self._shift_event(event, -self.x, -self.y)
        if not self._handle_event(local):
            return False
        for child in self._sub_components:
            if not child.dispatch_event(local):
                return False
        return True

    def force_mouse_exit(self) -> None:
        """递归清除整棵树的 hover/pressed 状态并触发 mouse_exit。

        页面切换时由 Router 调用，防止组件卡在 hover/pressed 状态。
        """
        if self._hovered or self._pressed:
            if self._pressed:
                self._pressed = False
                dummy = pygame.event.Event(
                    pygame.MOUSEBUTTONUP, {"pos": (-1, -1)})
                self._emit("mouse_up", dummy)
            self._hovered = False
            dummy = pygame.event.Event(pygame.MOUSEMOTION, {"pos": (-1, -1)})
            self._emit("mouse_exit", dummy)
        for child in self._sub_components:
            child.force_mouse_exit()

    def _handle_event(self, event: pygame.event.Event) -> bool:
        match event.type:
            case pygame.MOUSEMOTION:
                was_hovered = self._hovered
                self._hovered = self._in_rect(event.pos)
                if self._hovered and not was_hovered:
                    return self._emit("mouse_enter", event)
                elif not self._hovered and was_hovered:
                    return self._emit("mouse_exit", event)
                return True
            case pygame.MOUSEBUTTONDOWN:
                if self._in_rect(event.pos):
                    self._pressed = True
                    return self._emit("mouse_down", event)
                return True
            case pygame.MOUSEBUTTONUP:
                was_pressed = self._pressed
                self._pressed = False
                if not self._emit("mouse_up", event):
                    return False
                if was_pressed and self._in_rect(event.pos):
                    return self._emit("mouse_click", event)
                return True
            case pygame.KEYDOWN:
                key = event.key
                self._pressed_keys.add(key)
                return self._emit("key_down", event)
            case pygame.KEYUP:
                key = event.key
                self._pressed_keys.discard(key)
                return self._emit("key_up", event)
        return True

    def bind_on_mouse_enter(self, handler: Callable) -> None:
        """绑定鼠标进入事件。handler(event) -> bool。"""
        self._bind("mouse_enter", handler)

    def bind_on_mouse_exit(self, handler: Callable) -> None:
        """绑定鼠标离开事件。handler(event) -> bool。"""
        self._bind("mouse_exit", handler)

    def bind_on_mouse_down(self, handler: Callable) -> None:
        """绑定鼠标按下事件。handler(event) -> bool。"""
        self._bind("mouse_down", handler)

    def bind_on_mouse_up(self, handler: Callable) -> None:
        """绑定鼠标释放事件。handler(event) -> bool。"""
        self._bind("mouse_up", handler)

    def bind_on_mouse_click(self, handler: Callable) -> None:
        """绑定鼠标点击事件（按下+释放在组件内）。handler(event) -> bool。"""
        self._bind("mouse_click", handler)

    def bind_on_mouse_hold(self, handler: Callable) -> None:
        """绑定鼠标按住事件。handler(event) -> bool。"""
        self._bind("mouse_hold", handler)

    def bind_on_key_down(self, handler: Callable) -> None:
        """绑定按键按下事件。handler(event) -> bool。"""
        self._bind("key_down", handler)

    def bind_on_key_up(self, handler: Callable) -> None:
        """绑定按键释放事件。handler(event) -> bool。"""
        self._bind("key_up", handler)

    def bind_on_key_click(self, handler: Callable) -> None:
        """绑定按键点击事件。handler(event) -> bool。"""
        self._bind("key_click", handler)

    def bind_on_key_hold(self, handler: Callable) -> None:
        """绑定按键按住事件。handler(event) -> bool。"""
        self._bind("key_hold", handler)
