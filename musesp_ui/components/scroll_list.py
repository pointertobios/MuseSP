"""ScrollList 可滚动列表组件。"""

from collections.abc import Callable

import pygame

from musesp_ui.components.core import Component


class ScrollList(Component):
    """可滚动的列表组件。

    每项为调用方构造的 Component（需设置 _item_id 属性），
    通过 set_items() 设置。
    """

    def __init__(self, x: int = 0, y: int = 0, width: int = 0, height: int = 0,
                 item_height: int = 32):
        """:param item_height: 每项固定高度"""
        super().__init__(x=x, y=y, width=width, height=height)
        self.item_height = item_height
        self._scroll = 0
        self._max_scroll = 0
        self._on_select: Callable[[str], None] | None = None
        self._selected_id: str | None = None

    def set_items(self, items: list[Component]) -> None:
        """设置列表项。每项需设置 ``_item_id`` 作为标识。"""
        self._sub_components.clear()
        for i, comp in enumerate(items):
            comp.height = self.item_height
            self._sub_components.append(comp)
        self._max_scroll = max(
            0, len(items) * self.item_height - self.height)
        self._scroll = min(self._scroll, self._max_scroll)
        self._update_positions()

    def layout(self) -> None:
        super().layout()
        for child in self._sub_components:
            self._propagate_width(child, self.width)

    @staticmethod
    def _propagate_width(comp: Component, width: int) -> None:
        comp.width = width
        for sub in comp.sub_components:
            ScrollList._propagate_width(sub, width)

    def bind_on_select(self, handler: Callable[[str], None]) -> None:
        """绑定选中回调。``handler(item_id)``。重复点击已选中项不触发。"""
        self._on_select = handler

    # ---- 滚动位移直接写入子组件真实 y，默认绘制/调试/事件逻辑无需改动 ----

    def _update_positions(self) -> None:
        for i, child in enumerate(self._sub_components):
            child.y = i * self.item_height - self._scroll

    # ---- 绘制：仅需裁剪 ----

    def _draw_self(self, surface: pygame.Surface, draw_x: int, draw_y: int) -> None:
        pygame.draw.rect(surface, (30, 30, 30),
                         pygame.Rect(draw_x, draw_y, self.width, self.height))
        # 滚动条（仅在可滚动时显示）
        if self._max_scroll > 0:
            total_h = len(self._sub_components) * self.item_height
            bar_w = 4
            bar_h = max(20, self.height * self.height // total_h)
            track_h = self.height - bar_h
            bar_y = draw_y + (self._scroll * track_h // self._max_scroll
                              if self._max_scroll > 0 else 0)
            bar_x = draw_x + self.width - bar_w - 2
            pygame.draw.rect(surface, (100, 100, 100),
                             (bar_x, bar_y, bar_w, bar_h),
                             border_radius=bar_w // 2)

    def _draw_internal(self, surface: pygame.Surface,
                       offset_x: int, offset_y: int) -> None:
        draw_x = self.x + offset_x
        draw_y = self.y + offset_y
        self._draw_self(surface, draw_x, draw_y)
        old_clip = surface.get_clip()
        surface.set_clip(
            pygame.Rect(draw_x, draw_y, self.width, self.height))
        for child in self._sub_components:
            if -child.height < child.y < self.height:
                child._draw_internal(surface, draw_x, draw_y)
        surface.set_clip(old_clip)

    # ---- 事件：仅需可见性过滤 ----

    def dispatch_event(self, event: pygame.event.Event) -> bool:
        local = self._shift_event(event, -self.x, -self.y)
        if not self._handle_event(local):
            return False
        for child in self._sub_components:
            if -child.height < child.y < self.height:
                if not child.dispatch_event(local):
                    return False
        return True

    def _handle_event(self, event: pygame.event.Event) -> bool:
        if not super()._handle_event(event):
            return False
        if event.type == pygame.MOUSEWHEEL:
            self._scroll -= event.y * 24
            self._scroll = max(0, min(self._scroll, self._max_scroll))
            self._update_positions()
        elif event.type == pygame.MOUSEBUTTONDOWN and event.button == 1:
            if self._in_rect(event.pos) and self._on_select:
                for child in self._sub_components:
                    if child.y <= event.pos[1] < child.y + child.height:
                        item_id = getattr(child, "_item_id", "")
                        if item_id != self._selected_id:
                            self._selected_id = item_id
                            self._on_select(item_id)
                        return False
        return True
