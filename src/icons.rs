use sycamore::prelude::*;

pub fn icon(name: &str) -> View {
    match name {
        "plus" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="M12 5v14M5 12h14") } }
        }
        "search" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { circle(cx="11", cy="11", r="7") path(d="m20 20-3.4-3.4") } }
        }
        "pin" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="m14 4 6 6-3 1-4 4-1 5-2-6-4-4 5-1 4-4 1-3Z") } }
        }
        "settings" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { circle(cx="12", cy="12", r="3") path(d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1-2.8 2.8-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6v.2h-4V21a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1L4.2 17l.1-.1a1.7 1.7 0 0 0 .3-1.9A1.7 1.7 0 0 0 3 14H2.8v-4H3a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9L4.2 7 7 4.2l.1.1a1.7 1.7 0 0 0 1.9.3A1.7 1.7 0 0 0 10 3v-.2h4V3a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1L19.8 7l-.1.1a1.7 1.7 0 0 0-.3 1.9 1.7 1.7 0 0 0 1.6 1h.2v4H21a1.7 1.7 0 0 0-1.6 1Z") } }
        }
        "menu" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="M4 7h16M4 12h16M4 17h16") } }
        }
        "sources" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="M4 5h16v14H4zM8 9h8M8 13h5") } }
        }
        "copy" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { rect(x="8", y="8", width="11", height="11", rx="2") path(d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2") } }
        }
        "export" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="M12 3v12m0-12 4 4m-4-4L8 7M5 14v5h14v-5") } }
        }
        "trash" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="M4 7h16M9 7V4h6v3m3 0-1 13H7L6 7m4 4v5m4-5v5") } }
        }
        "send" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="m4 4 17 8-17 8 3-8-3-8Zm3 8h14") } }
        }
        "stop" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { rect(x="7", y="7", width="10", height="10", rx="1") } }
        }
        "spark" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="m12 2 1.5 5.5L19 9l-5.5 1.5L12 16l-1.5-5.5L5 9l5.5-1.5L12 2Zm7 13 .7 2.3L22 18l-2.3.7L19 21l-.7-2.3L16 18l2.3-.7L19 15Z") } }
        }
        "globe" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { circle(cx="12", cy="12", r="9") path(d="M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18") } }
        }
        "deep" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="M12 3v14m0 0-4-4m4 4 4-4M5 6h14M7 20h10") } }
        }
        "create" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="M4 20c3.5-1 6.2-3 8.3-6.1L19 4l1 1-9.9 6.7C7 13.8 5 16.5 4 20Z") path(d="m14.5 7.5 2 2M7.5 16.5l-2-2") } }
        }
        "key" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { circle(cx="8", cy="12", r="4") path(d="M12 12h9m-3 0v3m-3-3v2") } }
        }
        "check" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="m5 12 4 4L19 6") } }
        }
        "close" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="m6 6 12 12M18 6 6 18") } }
        }
        "external" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="M14 5h5v5M19 5l-9 9M18 13v6H5V6h6") } }
        }
        "retry" => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { path(d="M20 7v5h-5M4 17v-5h5m10.5 0A8 8 0 0 0 6 6.5L4 8m16 8-2 1.5A8 8 0 0 1 4.5 12") } }
        }
        _ => {
            view! { svg(viewBox="0 0 24 24", aria-hidden="true") { circle(cx="12", cy="12", r="8") } }
        }
    }
}
