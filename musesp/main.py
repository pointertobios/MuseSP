from musesp.pages.home import HomePage
from musesp_ui.application import Application


def main():
    app = Application("MuseSP", starts_with=HomePage())
    app.run()


if __name__ == "__main__":
    main()
