//! Test that the macOS provider actually works

use accessibility_mcp::platform::create_provider;

fn main() -> anyhow::Result<()> {
    println!("Testing macOS accessibility provider...");

    let provider = create_provider()?;
    println!("✓ Created provider");

    println!("\nAttempting to get root node...");
    match provider.get_root() {
        Ok(root) => {
            println!("✓ Got root node!");
            println!("  ID: {}", root.id.as_str());
            println!("  Role: {}", root.role);
            println!("  Name: {:?}", root.name);
            println!("  Children: {} nodes", root.children.len());
            println!("  Actions: {:?}", root.actions);
        }
        Err(e) => {
            println!("✗ Failed to get root: {}", e);
        }
    }

    Ok(())
}
