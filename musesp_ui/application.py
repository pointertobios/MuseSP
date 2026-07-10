import sys

import pygame

from musesp_ui.router import Page, Router


class Application:
    def __init__(self, name: str, starts_with: Page | None = None):
        if starts_with is None:
            sys.exit()
        self.name = name
        pygame.init()
        self.router = Router(starts_with)
        self.screen = pygame.display.set_mode((800, 600))
        self.router.current._root.width = self.screen.get_width()
        self.router.current._root.height = self.screen.get_height()
        self.router.current._root.layout()
        pygame.display.set_caption(name)

    def run(self):
        running = True
        while running:
            for event in pygame.event.get():
                if event.type == pygame.QUIT:
                    running = False
                self.router.dispatch_event(event)
            self.router.draw_pages(self.screen)
            pygame.display.flip()
        pygame.quit()
