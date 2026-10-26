import os
import re

# 1. Inject permissions into AndroidManifest.xml
manifest = "android/app/src/main/AndroidManifest.xml"
if os.path.exists(manifest):
    with open(manifest, "r") as f:
        content = f.read()
    if "android.permission.BLUETOOTH_SCAN" not in content:
        permissions = """
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
    <uses-permission android:name="android.permission.VIBRATE" />
    <uses-permission android:name="android.permission.CAMERA" />
    <uses-feature android:name="android.hardware.camera" android:required="false" />
    <uses-permission android:name="android.permission.BLUETOOTH" android:maxSdkVersion="30" />
    <uses-permission android:name="android.permission.BLUETOOTH_ADMIN" android:maxSdkVersion="30" />
    <uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" android:maxSdkVersion="30" />
    <uses-permission android:name="android.permission.BLUETOOTH_SCAN" android:usesPermissionFlags="neverForLocation" />
    <uses-permission android:name="android.permission.BLUETOOTH_CONNECT" />
    <uses-feature android:name="android.hardware.bluetooth_le" android:required="false" />
"""
        content = content.replace("<application", permissions + "\n    <application")
        with open(manifest, "w") as f:
            f.write(content)
        print("Successfully injected BLE and network permissions into AndroidManifest.xml")

# 2. Configure Groovy build.gradle if present
gradle_file = "android/app/build.gradle"
if os.path.exists(gradle_file):
    with open(gradle_file, "r") as f:
        content = f.read()

    # Enforce stable targetSdk 34 and minSdk 24
    content = re.sub(r'compileSdk\s+.*', 'compileSdk 34', content)
    content = re.sub(r'minSdkVersion\s+.*', 'minSdkVersion 24', content)
    content = re.sub(r'targetSdkVersion\s+.*', 'targetSdkVersion 34', content)

    with open(gradle_file, "w") as f:
        f.write(content)
    print("Successfully configured android/app/build.gradle with stable SDK 34")

# 3. Configure Kotlin DSL build.gradle.kts if present
kts_file = "android/app/build.gradle.kts"
if os.path.exists(kts_file):
    with open(kts_file, "r") as f:
        content = f.read()

    content = re.sub(r'compileSdk\s*=.*', 'compileSdk = 34', content)
    content = re.sub(r'minSdk\s*=.*', 'minSdk = 24', content)
    content = re.sub(r'targetSdk\s*=.*', 'targetSdk = 34', content)

    with open(kts_file, "w") as f:
        f.write(content)
    print("Successfully configured android/app/build.gradle.kts with stable SDK 34")
