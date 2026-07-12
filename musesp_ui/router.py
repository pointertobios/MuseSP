from collections.abc import Callable

import pygame

from musesp_ui.components.core import Component


class Page:
    def __init__(self):
        self._root = Component()
        self._router: "Router | None" = None

    def add_component(self, component: Component) -> None:
        self._root.add_sub_component(component)

    def hide_last(self) -> None:
        pass

    def build(self) -> None:
        pass

    def destroy(self) -> None:
        pass

    def on_hide(self) -> None:
        pass

    def on_activate(self) -> None:
        pass

    def full_shadow_promise(self) -> bool:
        return False

    def prepare_layout(self) -> None:
        """在根组件尺寸确定后、layout 之前调用。"""
        pass

    def dispatch_event(self, event: pygame.event.Event) -> None:
        self._root.dispatch_event(event)

    def draw(self, surface: pygame.Surface) -> None:
        self._root.draw(surface)
        self._root.draw_debug(surface)

    def draw_background(
        self, surface: pygame.Surface,
        background_bounds: tuple[int, int, int, int] | None = None,
        background: tuple[int, int, int] | pygame.Surface | None = None,
    ) -> None:
        """绘制页面背景。

        background_bounds 为 None 时覆盖整个窗口；
        background 为 None 时使用纯黑。
        """
        if background_bounds is None:
            x, y, w, h = 0, 0, self._root.width, self._root.height
        else:
            x, y, w, h = background_bounds
        if background is None:
            color = (0, 0, 0)
        elif isinstance(background, tuple):
            color = background
        else:
            surface.blit(background, (x, y))
            return
        pygame.draw.rect(surface, color, (x, y, w, h))


class PageToken:
    def __init__(self):
        self._value: object = None
        self._resolved: bool = False

    def get(self) -> object:
        return self._value

    def _resolve(self, value: object) -> None:
        self._value = value
        self._resolved = True


class Router:
    def __init__(self, initial: Page, app: object = None):
        self._stack: list[tuple[Page, PageToken]] = []
        self._screen: pygame.Surface | None = None
        self._app = app
        self._win_w: int = 0
        self._win_h: int = 0
        initial._router = self
        initial.build()
        self._stack.append((initial, PageToken()))

    def _init_page(self, page: Page) -> None:
        """初始化页面：注入 router、build、设置根尺寸、prepare_layout、layout。"""
        page._router = self
        page.build()
        page._root.width = self._win_w
        page._root.height = self._win_h
        page.prepare_layout()
        page._root.layout()

    def push(self, page: Page) -> PageToken:
        self.current.on_hide()
        self.current._root.force_mouse_exit()
        if self._screen is not None:
            self.current.draw_background(self._screen)
        self._init_page(page)
        token = PageToken()
        self._stack.append((page, token))
        return token

    def pop(self, value: object = None) -> None:
        if len(self._stack) <= 1:
            return
        page, token = self._stack.pop()
        token._resolve(value)
        page.destroy()
        self.current.on_activate()
        if self._screen is not None:
            self.current.draw_background(self._screen)

    def pop_then_else(
        self, fallback: Callable[[], Page], value: object = None,
    ) -> None:
        """弹出当前页；若栈将空，则调用 fallback 生成新页面压入。"""
        if len(self._stack) > 1:
            self.pop(value)
            return
        page, token = self._stack.pop()
        page.on_hide()
        page._root.force_mouse_exit()
        token._resolve(value)
        page.destroy()
        self._stack.clear()
        new_page = fallback()
        self._init_page(new_page)
        self._stack.append((new_page, PageToken()))
        if self._screen is not None:
            new_page.draw_background(self._screen)

    def pop_n_and_push(self, n: int, page: Page,
                       value: object = None) -> PageToken:
        """从栈顶弹出 n 个页面后压入新页面。"""
        if n <= 0:
            return self.push(page)
        if n >= len(self._stack):
            return self.clear_and_push(page)
        for _ in range(n):
            self.pop(value)
        return self.push(page)

    def clear_and_push(self, page: Page) -> PageToken:
        """清空页面栈（销毁所有页面），然后压入新页面。"""
        self.current.on_hide()
        self.current._root.force_mouse_exit()
        if self._screen is not None:
            self.current.draw_background(self._screen)
        for p, t in self._stack:
            p.destroy()
        self._stack.clear()
        self._init_page(page)
        if self._screen is not None:
            page.draw_background(self._screen)
        token = PageToken()
        self._stack.append((page, token))
        return token

    def dispatch_event(self, event: pygame.event.Event) -> None:
        self.current.dispatch_event(event)

    def draw_pages(self, surface: pygame.Surface) -> None:
        surface.fill((0, 0, 0))
        start = 0
        for i in range(len(self._stack) - 1, -1, -1):
            if self._stack[i][0].full_shadow_promise():
                start = i
                break
        for i in range(start, len(self._stack)):
            self._stack[i][0].draw(surface)

    @property
    def current(self) -> Page:
        return self._stack[-1][0]
