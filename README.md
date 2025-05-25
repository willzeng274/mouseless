# Mouseless

A mouseless navigation app for macOS that lets you click anywhere on screen using keyboard shortcuts.

## How It Works

1. **Tap Right ⌘** to show a transparent grid overlay covering your entire screen
2. **Type two letters** to select a main grid cell (like "AH" or "QJ")  
3. **Type one letter** to click precisely within that cell (like "A" or "K")
4. **Hold Shift** while selecting to right-click instead of left-click
5. **Press Escape** to hide the grid anytime

The grid uses keyboard-friendly two-letter combinations for fast navigation without taking your hands off the keyboard.

## Building & Running

### Prerequisites
- macOS (tested on macOS 14+)
- Rust toolchain
- Xcode Command Line Tools (for code signing)

### Quick Start
```bash
# Clone the repository (replace with your fork if you have one)
# git clone https://github.com/your-username/mouseless.git 
cd mouseless

# Build the release version
cargo build --release


# Run (requires accessibility permissions, see below)
./target/release/mouseless 
```

### Development
```bash
# Run in debug mode with logs
cargo run
```

## macOS Permissions & Code Signing

### Accessibility Permission (Required)

The app *must* have **Accessibility** permission to:
- Listen for global keyboard events (Right ⌘ tap, Escape key to hide).
- Programmatically move the mouse and send click events.

To grant permission:
1. Open **System Settings**.
2. Go to **Privacy & Security > Accessibility**.
3. Click the **+** button.
4. Navigate to the `target/release/` directory in the `mouseless` project folder and select the `mouseless` executable.
5. Ensure the toggle next to `mouseless` is enabled.

*You may need to restart the app after granting permissions for them to take effect.* 

### Code Signing for Local Use (Recommended)

While the app can run without code signing for personal use, macOS Gatekeeper might show warnings or prevent it from running easily, especially after transferring the app to another Mac or after updates. Signing it with a locally generated certificate can improve this experience.

**Steps to Create and Use a Local Signing Certificate:**

1.  **Open Keychain Access:**
    *   Press `⌘ + Space` to open Spotlight, type `Keychain Access`, and press Enter.

2.  **Create a New Certificate:**
    *   In Keychain Access, go to **Keychain Access > Certificate Assistant > Create a Certificate...** (should be in menubar)
    *   **Name:** Choose a descriptive name, e.g., `MouselessLocalSign` or `My Mac Developer`.
    *   **Identity Type:** Select **Self-Signed Root**.
    *   **Certificate Type:** Select **Code Signing**.
    *   **Let me override defaults:** Check this box.
    *   Click **Continue**.

3.  **Certificate Information (Serial Number & Validity):**
    *   You can leave the default serial number.
    *   Set **Validity Period (days):** `3650` (for 10 years, or adjust as needed).
    *   Click **Continue**.

4.  **Specify Key Usage Extension:**
    *   Ensure **Key Usage Extension** is included.
    *   Check the **Signature** box under "This extension is..."
    *   Click **Continue** until you reach the "Specify a Location for the Certificate" screen.

5.  **Specify Location:**
    *   **Keychain:** Select **login**.
    *   Click **Create**.

6.  **Set Trust Settings for the New Certificate:**
    *   In Keychain Access, find the certificate you just created (e.g., `MouselessLocalSign`) under the **My Certificates** category in the **login** keychain.
    *   Double-click the certificate to open its details.
    *   Expand the **Trust** section.
    *   Change **When using this certificate:** to **Always Trust**.
    *   Close the certificate details window (you may be prompted for your macOS user password to save changes).

7.  **Sign the Application Binary:**
    *   After building with `cargo build --release`, use the following command in your terminal, replacing `"MouselessLocalSign"` with the exact name of the certificate you created:
        ```bash
        codesign --force --deep --sign "MouselessLocalSign" ./target/release/Mouseless.app # or mouseless, whatever binary/app name you have
        ```

8.  **Verify Signature (Optional):**
    ```bash
    codesign --verify --verbose=4 ./target/release/mouseless
    ```
    And:
    ```bash
    spctl --assess --type execute --verbose ./target/release/mouseless
    ```
    The `spctl` command should output `accepted` if the signing was successful and the certificate is trusted.

Now your `mouseless` application is signed with your local certificate. This can help avoid some Gatekeeper warnings and makes it easier to run.

### Troubleshooting Permissions & Signing
If the app doesn't respond to keyboard shortcuts or won't launch:
1.  **Accessibility:** Double-check that `mouseless` is listed and **enabled** in System Settings > Privacy & Security > Accessibility. Try removing and re-adding it.
2.  **Restart:** Restart the app after granting/changing permissions or after signing.
3.  **Console Logs:** Open `Console.app` (from Spotlight) and search for "mouseless" or your app's bundle ID (if set) to see any error messages related to permissions or launching.
4.  **Signature Verification:** Use the `codesign --verify` and `spctl --assess` commands above to check the signature status.
5.  **Terminal Issues:** If you are using VSCode or Cursor, mouse clicking may not work if you ran the binaries from its integrated terminal. Not sure why this happens, but try to use a separate terminal app with permissions to fix the issue.

## Grid Layout

**Main Grid**: 12x12 using letter combinations
- Row chars: A,S,D,F,G,H,J,K,L,Q,W,E,R,T,Y,U,I,O,P,Z,X,C,V,B,N,M
- Col chars: H,J,K,L,Q,W,E,R,T,Y,A,S,D,F,G,U,I,O,P,Z,X,C,V,B,N,M

**Sub Grid**: 5x5 using single letters A-Y

## Dependencies

- `eframe/egui` - Cross-platform GUI
- `core-graphics` - macOS screen capture and mouse events  
- `mouse-rs` - Mouse positioning
- `objc/cocoa` - macOS window management
- `core-foundation` - macOS event tap system

## License

MIT 