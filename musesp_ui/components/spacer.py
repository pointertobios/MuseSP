"""Spacer —— 纯布局占位组件。"""

import pygame

from musesp_ui.components.core import Component


class Spacer(Component):
    """纯布局占位组件，不绘制内容、不响应事件。"""

    def __init__(self, width: int = 0, height: int = 0):
        super().__init__(width=width, height=height)

    def _in_rect(self, pos: tuple[int, int]) -> bool:
        return False

    def _draw_debug_internal(self, surface: pygame.Surface,
                             offset_x: int, offset_y: int) -> None:
        draw_x = self.x + offset_x
        draw_y = self.y + offset_y
        pygame.draw.rect(surface, (139, 0, 0),
                         pygame.Rect(draw_x, draw_y, self.width, self.height), 2)
        for child in self._sub_components:
            child._draw_debug_internal(surface, draw_x, draw_y)
