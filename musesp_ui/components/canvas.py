"""Canvas —— 独立画布组件。

提供独立的 pygame Surface，通过可调用对象在其上绘制，
_draw_self 时 blit 到窗口。
"""

from collections.abc import Callable

import pygame

from musesp_ui.components.core import Component


class Canvas(Component):
    """独立画布组件。

    :param draw_fn: 每帧调用的绘制函数 ``draw_fn(surface, dt)``，
                    surface 为组件内部画布，dt 为距离上一帧的毫秒数
    """

    def __init__(self, draw_fn: Callable[[pygame.Surface, float], None],
                 x: int = 0, y: int = 0, width: int = 0, height: int = 0):
        super().__init__(x=x, y=y, width=width, height=height)
        self._draw_fn = draw_fn
        self._canvas: pygame.Surface | None = None
        self._last_tick = pygame.time.get_ticks()

    def _ensure_canvas(self) -> None:
        if self._canvas is None or self._canvas.get_size() != (self.width, self.height):
            self._canvas = pygame.Surface((self.width, self.height))

    @property
    def canvas(self) -> pygame.Surface | None:
        """内部画布 Surface（只读）。"""
        return self._canvas

    def _draw_self(self, surface: pygame.Surface, draw_x: int, draw_y: int) -> None:
        self._ensure_canvas()
        now = pygame.time.get_ticks()
        dt = float(now - self._last_tick)
        self._last_tick = now
        self._draw_fn(self._canvas, dt)
        surface.blit(self._canvas, (draw_x, draw_y))
