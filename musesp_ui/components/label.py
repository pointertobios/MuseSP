"""Label —— 文本标签组件。"""

import pygame

from musesp_ui.components.core import Component
from musesp_ui.font import get_font


class Label(Component):
    """文本标签。

    在组件区域内水平和竖直居中渲染文本。
    """

    def __init__(self, text: str = "", x: int = 0, y: int = 0, width: int = 0,
                 height: int = 0, font_size: int = 24,
                 color: tuple[int, int, int] = (255, 255, 255)):
        """:param text: 显示文本
        :param font_size: 字号
        :param color: RGB 颜色
        """
        super().__init__(x=x, y=y, width=width, height=height)
        self.text = text
        self.font_size = font_size
        self.color = color

    def _draw_self(self, surface: pygame.Surface, draw_x: int, draw_y: int) -> None:
        font = get_font(self.font_size)
        text_surface = font.render(self.text, True, self.color)
        tw, th = text_surface.get_size()
        cx = draw_x + (self.width - tw) // 2
        cy = draw_y + (self.height - th) // 2
        surface.blit(text_surface, (cx, cy))
