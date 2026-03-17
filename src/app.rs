use leptos::prelude::*;
use leptos_meta::{HashedStylesheet, MetaTags, Title, provide_meta_context};
use leptos_router::{
    StaticSegment,
    components::{Route, Router, Routes},
};

use crate::pages::{config::ConfigPage, run::RunPage, schedule::SchedulePage};

pub fn shell(options: LeptosOptions) -> impl IntoView {
    view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8"/>
                <meta name="viewport" content="width=device-width, initial-scale=1"/>
                <HashedStylesheet options=options.clone()/>
                <AutoReload options=options.clone() />
                <HydrationScripts options=options.clone()/>
                <MetaTags/>
            </head>
            <body>
                <App/>
            </body>
        </html>
    }
}

#[component]
pub fn App() -> impl IntoView {
    provide_meta_context();

    view! {
        <Title text="Irrigation Scheduler"/>

        <Router>
            <header class="site-header">
                <div class="site-header__inner">
                    <span class="site-header__brand">"🌿 Irrigation Scheduler"</span>
                    <nav class="site-nav">
                        <a class="site-nav__link" href="/">"Schedule"</a>
                        <a class="site-nav__link" href="/config">"Configuration"</a>
                        <a class="site-nav__link" href="/run">"Force Run"</a>
                    </nav>
                </div>
            </header>
            <main class="site-main">
                <Routes fallback=|| "Page not found.".into_view()>
                    <Route path=StaticSegment("") view=SchedulePage/>
                    <Route path=StaticSegment("config") view=ConfigPage/>
                    <Route path=StaticSegment("run") view=RunPage/>
                </Routes>
            </main>
        </Router>
    }
}
