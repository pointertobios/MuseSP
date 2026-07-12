import pygame

_FONT_NAME = "uisc"

def get_font(size: int) -> pygame.font.Font:
    return pygame.font.SysFont(_FONT_NAME, size)
