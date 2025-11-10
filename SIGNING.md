# Code Signing Configuration

This document explains how to set up code signing for Live Transcribe.

## Overview

The build script supports two signing modes:

1. **Ad-hoc signing** (default) - For development only
   - Permissions reset after each build
   - No Apple Developer account needed
   - Free

2. **Developer ID signing** - For distribution
   - Permissions persist across updates
   - Requires Apple Developer Program ($99/year)
   - Required for macOS Sequoia (15.0+)

## Current Status

**Using: Ad-hoc signing (development mode)**

⚠️ With ad-hoc signing, macOS accessibility and microphone permissions will be lost after each new build because the binary hash changes.

## Setting Up Developer ID Signing

### Prerequisites

1. **Join Apple Developer Program**
   - Cost: $99 USD/year
   - Sign up at: https://developer.apple.com/programs/

2. **Create Developer ID Application Certificate**
   - Open Xcode → Settings → Accounts
   - Select your Apple ID → Manage Certificates
   - Click "+" → "Developer ID Application"
   - Or create via: https://developer.apple.com/account/resources/certificates

### Configure Build Script

#### Option 1: Environment Variables (Recommended)

```bash
# Set your Developer ID identity
export SIGNING_IDENTITY="Developer ID Application: Your Name (TEAM_ID)"

# Optional: Set up notarization profile for automatic notarization
export NOTARIZATION_PROFILE="YourProfileName"

# Build
./build_installer.sh 0.1.0
```

#### Option 2: Direct in Command

```bash
SIGNING_IDENTITY="Developer ID Application: Your Name (TEAM_ID)" ./build_installer.sh 0.1.0
```

### Find Your Signing Identity

List available Developer ID certificates:

```bash
security find-identity -v -p codesigning | grep "Developer ID Application"
```

Example output:
```
1) ABC123... "Developer ID Application: John Doe (TEAM123456)"
```

Use the full string in quotes as your `SIGNING_IDENTITY`.

### Set Up Notarization (Optional but Recommended)

Notarization removes security warnings for users on macOS 10.15+.

1. **Create App-Specific Password**
   - Go to: https://appleid.apple.com
   - Sign In → Security → App-Specific Passwords
   - Generate new password

2. **Store Credentials in Keychain**
   ```bash
   xcrun notarytool store-credentials "YourProfileName" \
       --apple-id "your@email.com" \
       --team-id "TEAM123456" \
       --password "app-specific-password"
   ```

3. **Use in Build**
   ```bash
   export SIGNING_IDENTITY="Developer ID Application: Your Name (TEAM123456)"
   export NOTARIZATION_PROFILE="YourProfileName"
   ./build_installer.sh 0.1.0
   ```

## GitHub Actions Configuration

To sign builds in GitHub Actions:

1. **Export Developer ID Certificate**
   ```bash
   # Export from Keychain Access as .p12 file with password
   ```

2. **Add GitHub Secrets**
   - `APPLE_CERTIFICATE_BASE64` - Base64 encoded .p12 file
   - `APPLE_CERTIFICATE_PASSWORD` - Certificate password
   - `APPLE_ID` - Your Apple ID email
   - `APPLE_TEAM_ID` - Your Team ID
   - `APPLE_APP_PASSWORD` - App-specific password

3. **Update workflow** (see `.github/workflows/release.yml`)

## Verification

After signing with Developer ID, verify:

```bash
# Check signature
codesign -dvv /Applications/LiveTranscribe.app

# Should show:
# Authority=Developer ID Application: Your Name (TEAM_ID)
# Signature=Developer ID
```

## Benefits of Developer ID Signing

- ✅ Permissions persist across updates
- ✅ No security warnings for users
- ✅ Required for macOS 15 Sequoia
- ✅ Professional appearance
- ✅ Automatic updates work properly

## Cost-Benefit Analysis

**Ad-hoc (Free)**
- Good for: Personal use, development
- Bad for: Distribution to others

**Developer ID ($99/year)**
- Good for: Public distribution, production
- Required for: macOS 15+, serious projects

For this project, if distributing to users, Developer ID is highly recommended.
