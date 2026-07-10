import pygame

from musesp_ui.components import Component


class Page:
    def __init__(self):
        self._root = Component()

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

    def dispatch_event(self, event: pygame.event.Event) -> None:
        self._root.dispatch_event(event)

    def draw(self, surface: pygame.Surface) -> None:
        self._root.draw(surface)


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
    def __init__(self, initial: Page):
        self._stack: list[tuple[Page, PageToken]] = []
        initial.build()
        self._stack.append((initial, PageToken()))

    def push(self, page: Page) -> PageToken:
        self.current.on_hide()
        page.build()
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

    def dispatch_event(self, event: pygame.event.Event) -> None:
        self.current.dispatch_event(event)

    def draw_pages(self, surface: pygame.Surface) -> None:
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
