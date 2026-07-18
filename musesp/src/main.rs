mod gameplay;
mod gameplay_page;
mod menu_page;
mod pages;

use musesp_ui::application::Application;
use pages::home::HomePage;

fn main() {
    Application::run("MuseSP", HomePage::new());
}
