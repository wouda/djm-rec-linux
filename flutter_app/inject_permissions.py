import os
import re
import shutil

# 1. Copy release keystore to android/app if keystore exists
keystore_src = "keystore/release.keystore"
keystore_dest = "android/app/release.keystore"
if os.path.exists(keystore_src) and os.path.exists("android/app"):
    shutil.copyfile(keystore_src, keystore_dest)
    print(f"Copied {keystore_src} -> {keystore_dest}")

# 2. Inject permissions into AndroidManifest.xml
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

# 3. Configure Groovy build.gradle if present
gradle_file = "android/app/build.gradle"
if os.path.exists(gradle_file):
    with open(gradle_file, "r") as f:
        content = f.read()

    # Enforce stable targetSdk 34 and minSdk 24
    content = re.sub(r'compileSdk\s+.*', 'compileSdk 34', content)
    content = re.sub(r'minSdkVersion\s+.*', 'minSdkVersion 24', content)
    content = re.sub(r'targetSdkVersion\s+.*', 'targetSdkVersion 34', content)

    if 'signingConfigs {' not in content:
        signing_block = """
    signingConfigs {
        release {
            storeFile file("release.keystore")
            storePassword "djmcompanion"
            keyAlias "djm_companion"
            keyPassword "djmcompanion"
            v1SigningEnabled true
            v2SigningEnabled true
        }
    }
"""
        content = content.replace("buildTypes {", signing_block + "\n    buildTypes {")

    content = re.sub(r'signingConfig\s+signingConfigs\.debug', 'signingConfig signingConfigs.release', content)

    with open(gradle_file, "w") as f:
        f.write(content)
    print("Successfully configured android/app/build.gradle with stable SDK 34 and release signing")

# 4. Configure Kotlin DSL build.gradle.kts if present
kts_file = "android/app/build.gradle.kts"
if os.path.exists(kts_file):
    with open(kts_file, "r") as f:
        content = f.read()

    content = re.sub(r'compileSdk\s*=.*', 'compileSdk = 34', content)
    content = re.sub(r'minSdk\s*=.*', 'minSdk = 24', content)
    content = re.sub(r'targetSdk\s*=.*', 'targetSdk = 34', content)

    if 'signingConfigs {' not in content:
        signing_block = """
    signingConfigs {
        create("release") {
            storeFile = file("release.keystore")
            storePassword = "djmcompanion"
            keyAlias = "djm_companion"
            keyPassword = "djmcompanion"
            enableV1Signing = true
            enableV2Signing = true
        }
    }
"""
        content = content.replace("buildTypes {", signing_block + "\n    buildTypes {")

    content = re.sub(r'signingConfig\s*=.*signingConfigs\.getByName\("debug"\)', 'signingConfig = signingConfigs.getByName("release")', content)
    content = re.sub(r'signingConfig\s*=\s*signingConfigs\.debug', 'signingConfig = signingConfigs.getByName("release")', content)

    with open(kts_file, "w") as f:
        f.write(content)
    print("Successfully configured android/app/build.gradle.kts with stable SDK 34 and release signing")
