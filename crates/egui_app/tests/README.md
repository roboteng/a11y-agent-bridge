# AccessKit Integration Tests

This directory contains integration tests that verify AccessKit is properly exposing egui widgets through the macOS Accessibility API.

## Purpose

These tests prevent regressions where changes to the codebase break AccessKit integration, causing egui widgets to become inaccessible via the Accessibility API.

## Tests

### 1. `test_accesskit_exposes_widgets`
Verifies that egui widgets (window, buttons, checkbox) are visible in the accessibility tree immediately after app startup.

### 2. `test_slider_is_accessible_and_interactive`
Tests that the slider widget is discoverable and that increment/decrement actions work correctly via the MCP protocol.

### 3. `test_accesskit_lazy_init_is_disabled`
Ensures that AccessKit is initialized immediately (via `ctx.enable_accesskit()`) rather than waiting for a "real" accessibility client like VoiceOver.

## Running the Tests

These tests are marked with `#[ignore]` because they:
- Start a GUI application
- Take several seconds to run
- Use `#[serial]` to run sequentially (avoiding socket conflicts)

### Run all AccessKit tests:
```bash
cargo test -p egui_app --features a11y_mcp -- --ignored
```

### Run a specific test:
```bash
cargo test -p egui_app --features a11y_mcp -- --ignored test_accesskit_exposes_widgets
```

### Include in CI:
```bash
# On macOS CI runners only
cargo test -p egui_app --features a11y_mcp -- --ignored
```

## Requirements

- **macOS only**: These tests use the macOS Accessibility API
- **Feature flag**: Must run with `--features a11y_mcp`
- **Sequential execution**: Tests use `#[serial]` attribute to avoid socket conflicts
- **GUI environment**: Requires a display (may fail in headless CI)

## What Gets Tested

1. **AccessKit initialization**: Verifies `ctx.enable_accesskit()` is called and working
2. **Widget discovery**: Confirms egui widgets appear in the accessibility tree
3. **Interaction**: Tests that accessibility actions (increment/decrement) work
4. **MCP protocol**: Validates the full chain: egui → AccessKit → macOS API → MCP server

## Troubleshooting

### Test fails with "Only found 1 node"
This means AccessKit is not exposing widgets. Likely causes:
- `ctx.enable_accesskit()` is not being called in `DemoApp::update()`
- AccessKit feature is disabled in Cargo.toml
- AccessKit initialization is broken

### Test fails with "Socket file not found"
The egui_app didn't start properly. Check:
- Build succeeded
- No other instance is running
- Sufficient startup delay (5 seconds)

### Test fails with "AXSlider not found"
The slider widget isn't in the accessibility tree:
- Verify the egui app still has a slider widget
- Check that AccessKit supports sliders in this version
- Ensure the widget is actually rendered (not hidden/conditional)

## Adding New Tests

When adding new egui widgets to the demo app:

1. Add a test case to verify it's accessible
2. Check both discovery (it appears in the tree) and interaction (actions work)
3. Mark the test with `#[ignore]` and document in this README
4. Update the expected node count in `test_accesskit_exposes_widgets`

## Example Test Output

```
running 3 tests
test accesskit_tests::test_accesskit_exposes_widgets ... ok
test accesskit_tests::test_slider_is_accessible_and_interactive ... ok
test accesskit_tests::test_accesskit_lazy_init_is_disabled ... ok

test result: ok. 3 passed; 0 failed; 0 ignored
```

## Related Files

- `src/main.rs`: Contains `ctx.enable_accesskit()` call
- `../../../accessibility_mcp/`: MCP server implementation
- `Cargo.toml`: Feature flag `a11y_mcp`
