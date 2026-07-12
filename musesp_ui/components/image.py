"""Image —— 图片显示组件。"""

from enum import Enum, auto

import pygame

from musesp_ui.components.core import Component


class ImageMode(Enum):
    """图片显示模式。"""
    COVER = auto()      # 缩放填满
    CENTERED = auto()   # 不缩放，居中
    KEEP_RATE = auto()  # 根据另一方向保持比例；双向均为 KEEP_RATE 则使用原图大小
    ORIGIN = auto()     # 使用原图尺寸


class Image(Component):
    """图片组件，水平和竖直方向各有一种显示模式。"""

    def __init__(self, path: str,
                 x: int = 0, y: int = 0, width: int = 0, height: int = 0,
                 h_mode: ImageMode = ImageMode.CENTERED,
                 v_mode: ImageMode = ImageMode.CENTERED):
        super().__init__(x=x, y=y, width=width, height=height)
        self._surface = pygame.image.load(path) if path else pygame.Surface((1, 1))
        self.h_mode = h_mode
        self.v_mode = v_mode

    def set_image(self, path: str) -> None:
        """更换显示的图片。"""
        self._surface = pygame.image.load(path)

    def _draw_self(self, surface: pygame.Surface, draw_x: int, draw_y: int) -> None:
        orig = self._surface
        iw, ih = orig.get_size()
        dw, dh = self._display_size(iw, ih)

        if (dw, dh) != (iw, ih):
            img = pygame.transform.scale(orig, (dw, dh))
        else:
            img = orig

        # COVER / ORIGIN → 原点；CENTERED / KEEP_RATE → 居中
        dx = draw_x
        dy = draw_y
        if self.h_mode in (ImageMode.CENTERED, ImageMode.KEEP_RATE):
            dx += (self.width - dw) // 2
        if self.v_mode in (ImageMode.CENTERED, ImageMode.KEEP_RATE):
            dy += (self.height - dh) // 2
        surface.blit(img, (dx, dy))

    def _display_size(self, iw: int, ih: int) -> tuple[int, int]:
        """根据两个方向的模式计算最终显示尺寸。"""
        h_mode, v_mode = self.h_mode, self.v_mode

        # --- 宽度 ---
        if h_mode == ImageMode.COVER:
            dw = self.width
        elif h_mode == ImageMode.ORIGIN:
            dw = iw
        elif h_mode == ImageMode.KEEP_RATE:
            if v_mode == ImageMode.COVER:
                dw = max(1, int(iw * self.height / ih))
            else:
                dw = iw
        else:  # CENTERED
            dw = iw

        # --- 高度 ---
        if v_mode == ImageMode.COVER:
            dh = self.height
        elif v_mode == ImageMode.ORIGIN:
            dh = ih
        elif v_mode == ImageMode.KEEP_RATE:
            if h_mode == ImageMode.COVER:
                dh = max(1, int(ih * self.width / iw))
            else:
                dh = ih
        else:  # CENTERED
            dh = ih

        return dw, dh
