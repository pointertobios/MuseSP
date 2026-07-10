import pygame

_FONT_NAME = "Sarasa UI SC"


def get_font(size: int) -> pygame.font.Font:
    return pygame.font.SysFont(_FONT_NAME, size)
