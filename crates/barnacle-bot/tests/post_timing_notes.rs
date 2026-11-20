use barnacle_bot::release_notes::NotesError;
use barnacle_bot::release_notes::all;
use barnacle_bot::release_notes::find;
use barnacle_bot::release_notes::problems;

#[test]
fn the_0_4_0_notes_tell_of_sign_up_timing() -> Result<(), NotesError> {
    assert_eq!(env!("CARGO_PKG_VERSION"), "0.4.0");
    let releases = all()?;
    let release = find(&releases, "0.4.0").expect("0.4.0 has notes");
    assert_eq!(problems(std::slice::from_ref(release)), Vec::new());
    let rendered = release.render();
    for phrase in ["1 to 7 days", "4 hours", "in the past"] {
        assert!(rendered.contains(phrase), "{phrase}");
    }
    Ok(())
}
