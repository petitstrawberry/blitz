use blitz_test_harness::{Harness, mouse_pointer_event};
use blitz_traits::events::{MouseEventButton, MouseEventButtons, UiEvent};

const HTML: &str = r#"<!doctype html>
<style>html,body{margin:0}p{margin:0;font:32px monospace;line-height:40px}</style>
<p id="text">ABCDEFGH</p>"#;

fn point_for_caret(harness: &Harness, caret: usize) -> Option<(f32, f32)> {
    let node = harness.base().query_selector("#text").ok()??;
    let rect = harness.base().get_client_bounding_rect(node)?;
    let scroll = harness.base().viewport_scroll();
    let y = (rect.y + rect.height * 0.5 + scroll.y) as f32;
    let start = (rect.x + scroll.x).max(0.0) as f32;
    let end = (rect.x + rect.width.min(400.0) + scroll.x) as f32;
    let mut x = start;
    while x <= end {
        if harness
            .base()
            .find_text_position(x, y)
            .is_some_and(|(_, offset)| offset == caret)
        {
            return Some((x, y));
        }
        x += 0.25;
    }
    None
}

#[test]
fn viewport_scroll_does_not_shift_selection_hit_testing() {
    let mut harness = Harness::from_html(
        r#"<!doctype html>
        <style>html,body{margin:0}#spacer{height:700px}p{margin:0;font:32px monospace;line-height:40px}</style>
        <div id="spacer"></div><p id="text">ABCDEFGH</p>"#,
    );
    harness
        .base_mut()
        .set_viewport_scroll(blitz_dom::Point { x: 0.0, y: 600.0 });

    let (Some(first_right_half), Some(last_right_half)) =
        (point_for_caret(&harness, 1), point_for_caret(&harness, 7))
    else {
        panic!("scrolled text did not produce usable caret positions");
    };
    harness.drag(first_right_half, last_right_half, 1);

    let selected = harness.base().get_selected_text().unwrap_or_default();
    assert_eq!(selected, "ABCDEFG");
}

#[test]
fn release_position_is_committed_as_final_selection_endpoint() {
    let mut harness = Harness::from_html(HTML);
    let (Some(start), Some(intermediate), Some(release)) = (
        point_for_caret(&harness, 0),
        point_for_caret(&harness, 3),
        point_for_caret(&harness, 7),
    ) else {
        eprintln!("skipping: no usable text layout");
        return;
    };

    harness.dispatch(UiEvent::PointerDown(mouse_pointer_event(start.0, start.1)));
    harness.dispatch(UiEvent::PointerMove(mouse_pointer_event(
        intermediate.0,
        intermediate.1,
    )));
    let before_release = harness.base().get_selected_text().unwrap_or_default().len();

    let mut up = mouse_pointer_event(release.0, release.1);
    up.buttons = MouseEventButtons::None;
    up.button = MouseEventButton::Main;
    harness.dispatch(UiEvent::PointerUp(up));

    let selected = harness.base().get_selected_text().unwrap_or_default();
    assert!(
        selected.len() > before_release,
        "release endpoint was not committed: before={before_release}, selected={selected:?}"
    );
    assert!(
        selected.ends_with('G'),
        "expected the grapheme under the release point, got {selected:?}"
    );
}

#[test]
fn endpoint_graphemes_do_not_depend_on_drag_direction() {
    let mut forward = Harness::from_html(HTML);
    let (Some(first_right_half), Some(last_left_half)) =
        (point_for_caret(&forward, 1), point_for_caret(&forward, 7))
    else {
        eprintln!("skipping: no usable text layout");
        return;
    };
    forward.drag(first_right_half, last_left_half, 1);
    let forward_text = forward.base().get_selected_text().unwrap_or_default();

    let mut reverse = Harness::from_html(HTML);
    reverse.drag(last_left_half, first_right_half, 1);
    let reverse_text = reverse.base().get_selected_text().unwrap_or_default();

    assert_eq!(forward_text, reverse_text);
    assert!(
        forward_text.starts_with('A'),
        "the first grapheme under the pointer was dropped: {forward_text:?}"
    );
}
