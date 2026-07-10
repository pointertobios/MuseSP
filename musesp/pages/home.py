from musesp_ui.components import Button, Constraintable, Label
from musesp_ui.router import Page


class HomePage(Page):
    def full_shadow_promise(self) -> bool:
        return True

    def hide_last(self) -> None:
        pass

    def build(self) -> None:
        title = Label("MuseSP", font_size=72, color=(255, 255, 255))
        title.v_constraint = Constraintable.MINIMUM
        title.h_constraint = Constraintable.MINIMUM
        title.min_height = 120
        title.min_width = 400
        self.add_component(title)

        btn_start = Button("开始")
        btn_start.v_constraint = Constraintable.MINIMUM
        btn_start.h_constraint = Constraintable.MAXIMUM
        btn_start.min_height = 50
        btn_start.min_width = 200
        self.add_component(btn_start)

        btn_settings = Button("设置")
        btn_settings.v_constraint = Constraintable.MINIMUM
        btn_settings.h_constraint = Constraintable.MAXIMUM
        btn_settings.min_height = 50
        btn_settings.min_width = 200
        self.add_component(btn_settings)

        btn_exit = Button("退出")
        btn_exit.v_constraint = Constraintable.MINIMUM
        btn_exit.h_constraint = Constraintable.MAXIMUM
        btn_exit.min_height = 50
        btn_exit.min_width = 200
        self.add_component(btn_exit)
