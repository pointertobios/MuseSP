from collections.abc import Callable
from enum import Enum, auto

import pygame

from musesp_ui.font import get_font


class Constraintable(Enum):
    NONE = auto()
    MINIMUM = auto()
    MAXIMUM = auto()


class Direction(Enum):
    VERTICAL = auto()
    HORIZONTAL = auto()


class Component:
    def __init__(self, x: int = 0, y: int = 0, width: int = 0, height: int = 0):
        self.x = x
        self.y = y
        self.width = width
        self.height = height
        self.min_width = 0
        self.min_height = 0
        self.h_constraint = Constraintable.NONE
        self.v_constraint = Constraintable.NONE
        self.layout_direction = Direction.VERTICAL
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
        return self._sub_components

    def add_sub_component(self, component: "Component") -> None:
        self._sub_components.append(component)

    def layout(self) -> None:
        if self.layout_direction == Direction.VERTICAL:
            self._layout_axis("v", "y", "height", "min_height",
                              "v_constraint", "width")
        else:
            self._layout_axis("h", "x", "width", "min_width",
                              "h_constraint", "height")

    def _layout_axis(self, axis: str, pos_attr: str, size_attr: str,
                     min_attr: str, constraint_attr: str, cross_attr: str) -> None:
        parent_size = getattr(self, size_attr)
        constrained = [c for c in self._sub_components
                       if getattr(c, constraint_attr) != Constraintable.NONE]
        if not constrained:
            return

        total_min = sum(getattr(c, min_attr) for c in constrained)
        remaining = parent_size - total_min
        maximums = [c for c in constrained if getattr(
            c, constraint_attr) == Constraintable.MAXIMUM]
        share = max(0, remaining // max(len(maximums), 1))

        pos = 0
        cross = getattr(self, cross_attr)
        for child in self._sub_components:
            constraint = getattr(child, constraint_attr)
            if constraint == Constraintable.NONE:
                continue
            setattr(child, pos_attr, pos)
            setattr(child, cross_attr, cross)
            if constraint == Constraintable.MINIMUM:
                size = getattr(child, min_attr)
            else:
                size = getattr(child, min_attr) + share
            setattr(child, size_attr, size)
            pos += size

    def _bind(self, event: str, handler: Callable) -> None:
        self._handlers[event].append(handler)

    def _in_rect(self, pos: tuple[int, int]) -> bool:
        x, y = pos
        return 0 <= x <= self.width and 0 <= y <= self.height

    def _emit(self, name: str, event: pygame.event.Event) -> None:
        for handler in self._handlers[name]:
            handler(event)

    def _shift_event(self, event: pygame.event.Event, dx: int, dy: int) -> pygame.event.Event:
        if event.type in (pygame.MOUSEMOTION, pygame.MOUSEBUTTONDOWN, pygame.MOUSEBUTTONUP):
            attrs = dict(event.dict)
            ex, ey = event.pos
            attrs["pos"] = (ex + dx, ey + dy)
            return pygame.event.Event(event.type, attrs)
        return event

    def draw(self, surface: pygame.Surface) -> None:
        self._draw_internal(surface, 0, 0)

    def _draw_internal(self, surface: pygame.Surface, offset_x: int, offset_y: int) -> None:
        draw_x = self.x + offset_x
        draw_y = self.y + offset_y
        self._draw_self(surface, draw_x, draw_y)
        for child in self._sub_components:
            child._draw_internal(surface, draw_x, draw_y)

    def _draw_self(self, surface: pygame.Surface, draw_x: int, draw_y: int) -> None:
        pass

    def dispatch_event(self, event: pygame.event.Event) -> None:
        local = self._shift_event(event, -self.x, -self.y)
        self._handle_event(local)
        for child in self._sub_components:
            child.dispatch_event(local)

    def _handle_event(self, event: pygame.event.Event) -> None:
        match event.type:
            case pygame.MOUSEMOTION:
                was_hovered = self._hovered
                self._hovered = self._in_rect(event.pos)
                if self._hovered and not was_hovered:
                    self._emit("mouse_enter", event)
                elif not self._hovered and was_hovered:
                    self._emit("mouse_exit", event)
            case pygame.MOUSEBUTTONDOWN:
                if self._in_rect(event.pos):
                    self._pressed = True
                    self._emit("mouse_down", event)
            case pygame.MOUSEBUTTONUP:
                was_pressed = self._pressed
                self._pressed = False
                self._emit("mouse_up", event)
                if was_pressed and self._in_rect(event.pos):
                    self._emit("mouse_click", event)
            case pygame.KEYDOWN:
                key = event.key
                self._pressed_keys.add(key)
                self._emit("key_down", event)
            case pygame.KEYUP:
                key = event.key
                self._pressed_keys.discard(key)
                self._emit("key_up", event)

    def bind_on_mouse_enter(self, handler: Callable) -> None:
        self._bind("mouse_enter", handler)

    def bind_on_mouse_exit(self, handler: Callable) -> None:
        self._bind("mouse_exit", handler)

    def bind_on_mouse_down(self, handler: Callable) -> None:
        self._bind("mouse_down", handler)

    def bind_on_mouse_up(self, handler: Callable) -> None:
        self._bind("mouse_up", handler)

    def bind_on_mouse_click(self, handler: Callable) -> None:
        self._bind("mouse_click", handler)

    def bind_on_mouse_hold(self, handler: Callable) -> None:
        self._bind("mouse_hold", handler)

    def bind_on_key_down(self, handler: Callable) -> None:
        self._bind("key_down", handler)

    def bind_on_key_up(self, handler: Callable) -> None:
        self._bind("key_up", handler)

    def bind_on_key_click(self, handler: Callable) -> None:
        self._bind("key_click", handler)

    def bind_on_key_hold(self, handler: Callable) -> None:
        self._bind("key_hold", handler)


class Label(Component):
    def __init__(self, text: str = "", x: int = 0, y: int = 0, width: int = 0, height: int = 0,
                 font_size: int = 24,
                 color: tuple[int, int, int] = (255, 255, 255)):
        super().__init__(x=x, y=y, width=width, height=height)
        self.text = text
        self.font_size = font_size
        self.color = color

    def _draw_self(self, surface: pygame.Surface, draw_x: int, draw_y: int) -> None:
        font = get_font(self.font_size)
        text_surface = font.render(self.text, True, self.color)
        surface.blit(text_surface, (draw_x, draw_y))


class Button(Component):
    def __init__(self, text: str = "", x: int = 0, y: int = 0, width: int = 120,
                 height: int = 40, font_size: int = 24):
        super().__init__(x=x, y=y, width=width, height=height)
        self.text = text
        self.font_size = font_size
        font = get_font(font_size)
        tw, th = font.size(text)
        pad = 4
        lw = tw + pad * 2
        lh = th + pad * 2
        label_x = (width - lw) // 2
        label_y = (height - lh) // 2
        self._label = Label(text, x=label_x, y=label_y, width=lw, height=lh,
                            font_size=font_size)
        self._label.h_constraint = Constraintable.NONE
        self._label.v_constraint = Constraintable.NONE
        self.add_sub_component(self._label)

    def _draw_self(self, surface: pygame.Surface, draw_x: int, draw_y: int) -> None:
        rect = pygame.Rect(draw_x, draw_y, self.width, self.height)
        if self._pressed:
            bg_color = (100, 100, 100)
        elif self._hovered:
            bg_color = (140, 140, 140)
        else:
            bg_color = (80, 80, 80)
        pygame.draw.rect(surface, bg_color, rect)
