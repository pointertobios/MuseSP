"""ImageButton —— 图片按钮组件。"""

from collections.abc import Callable

import pygame

from musesp_ui.components.core import Component, Constraintable, Direction
from musesp_ui.components.image import Image, ImageMode
from musesp_ui.components.label import Label
from musesp_ui.font import get_font

_DUMMY_EVENT = pygame.event.Event(pygame.USEREVENT)


class ImageButton(Component):
    """图片按钮。

    内部包含一个 Image（竖直 COVER、水平 KEEP_RATE），可选文本。
    根据 ``layout_direction`` 自动排布图文位置。

    支持 enable/disable 状态及对应事件。
    """

    def __init__(self, path: str = "", text: str = "",
                 x: int = 0, y: int = 0,
                 width: int = 120, height: int = 40,
                 font_size: int = 18):
        super().__init__(x=x, y=y, width=width, height=height)
        self.enabled = True
        self._handlers["enable"] = []
        self._handlers["disable"] = []
        self._overlay: pygame.Surface | None = None
        self.layout_direction = Direction.HORIZONTAL

        if text:
            self._image = Image(path, h_mode=ImageMode.KEEP_RATE,
                                v_mode=ImageMode.COVER)
            font = get_font(font_size)
            tw, th = font.size(text)
            self._label = Label(text, font_size=font_size,
                                color=(220, 220, 220))
            if self.layout_direction == Direction.HORIZONTAL:
                self._image.h_constraint = Constraintable.MAXIMUM
                self._image.v_constraint = Constraintable.NONE
                self._image.min_width = height
                self._image.max_width = height
                self._label.h_constraint = Constraintable.MINIMUM
                self._label.v_constraint = Constraintable.NONE
                self._label.min_width = tw + 8
            else:
                self._image.v_constraint = Constraintable.MAXIMUM
                self._image.h_constraint = Constraintable.NONE
                self._image.min_height = width
                self._image.max_height = width
                self._label.v_constraint = Constraintable.MINIMUM
                self._label.h_constraint = Constraintable.NONE
                self._label.min_height = th + 4
            self.add_sub_component(self._image)
            self.add_sub_component(self._label)
        else:
            self._image = Image(path, h_mode=ImageMode.KEEP_RATE,
                                v_mode=ImageMode.COVER)
            if self.layout_direction == Direction.HORIZONTAL:
                self._image.h_constraint = Constraintable.MAXIMUM
                self._image.v_constraint = Constraintable.NONE
                self._image.min_width = height
                self._image.max_width = height
            else:
                self._image.v_constraint = Constraintable.MAXIMUM
                self._image.h_constraint = Constraintable.NONE
                self._image.min_height = width
                self._image.max_height = width
            self.add_sub_component(self._image)

    def set_image(self, path: str) -> None:
        """更换按钮图片。"""
        self._image.set_image(path)

    # ---- enable / disable ----

    def enable(self) -> None:
        """启用按钮，触发 ``on_enable`` 事件。"""
        if not self.enabled:
            self.enabled = True
            self._emit("enable", _DUMMY_EVENT)

    def disable(self) -> None:
        """禁用按钮，清除 hover/pressed，触发 ``on_disable`` 事件。"""
        if self.enabled:
            self.enabled = False
            self._hovered = False
            self._pressed = False
            self._emit("disable", _DUMMY_EVENT)

    def bind_on_enable(self, handler: Callable) -> None:
        """绑定启用事件。``handler(event) -> bool``。"""
        self._bind("enable", handler)

    def bind_on_disable(self, handler: Callable) -> None:
        """绑定禁用事件。``handler(event) -> bool``。"""
        self._bind("disable", handler)

    # ---- 绘制 / 事件 ----

    def _draw_self(self, surface: pygame.Surface, draw_x: int, draw_y: int) -> None:
        rect = pygame.Rect(draw_x, draw_y, self.width, self.height)
        if not self.enabled:
            bg_color = (80, 80, 80)
        elif self._pressed:
            bg_color = (100, 100, 100)
        elif self._hovered:
            bg_color = (140, 140, 140)
        else:
            bg_color = (80, 80, 80)
        pygame.draw.rect(surface, bg_color, rect)
        if not self.enabled:
            self._ensure_overlay()
            surface.blit(self._overlay, (draw_x, draw_y))

    def _ensure_overlay(self) -> None:
        if (self._overlay is not None
                and self._overlay.get_size() == (self.width, self.height)):
            return
        overlay = pygame.Surface((self.width, self.height), pygame.SRCALPHA)
        overlay.fill((0, 0, 0, 128))
        self._overlay = overlay

    def _handle_event(self, event: pygame.event.Event) -> bool:
        if not self.enabled:
            return True
        return super()._handle_event(event)
