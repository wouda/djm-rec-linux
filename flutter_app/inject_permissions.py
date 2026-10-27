import os
import re

# 1. Inject permissions and cleartext traffic into AndroidManifest.xml
manifest = "android/app/src/main/AndroidManifest.xml"
if os.path.exists(manifest):
    with open(manifest, "r") as f:
        content = f.read()

    # Enable cleartext HTTP / WS traffic (required for LAN / hotspot connection to djm-box)
    if "android:usesCleartextTraffic" not in content:
        content = content.replace("<application", '<application\n        android:usesCleartextTraffic="true"')

    # Inject required permissions if missing
    if "android.permission.INTERNET" not in content:
        permissions = """
    <uses-permission android:name="android.permission.INTERNET" />
    <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />
    <uses-permission android:name="android.permission.ACCESS_WIFI_STATE" />
    <uses-permission android:name="android.permission.CHANGE_WIFI_MULTICAST_STATE" />
    <uses-permission android:name="android.permission.VIBRATE" />
    <uses-permission android:name="android.permission.CAMERA" />
    <uses-feature android:name="android.hardware.camera" android:required="false" />
"""
        content = content.replace("<application", permissions + "\n    <application")

    with open(manifest, "w") as f:
        f.write(content)
    print("Successfully injected permissions and cleartext traffic into AndroidManifest.xml")

# 2. Configure android/local.properties to set SDK baselines
props_file = "android/local.properties"
props_lines = []
if os.path.exists(props_file):
    with open(props_file, "r") as f:
        props_lines = f.readlines()

props_lines = [
    line for line in props_lines
    if not any(k in line for k in ["flutter.minSdkVersion", "flutter.compileSdkVersion", "flutter.targetSdkVersion"])
]
props_lines.append("flutter.minSdkVersion=24\n")
props_lines.append("flutter.compileSdkVersion=34\n")
props_lines.append("flutter.targetSdkVersion=34\n")

with open(props_file, "w") as f:
    f.writelines(props_lines)
print("Successfully set SDK properties in android/local.properties")

# 3. Configure Groovy build.gradle if present
gradle_file = "android/app/build.gradle"
if os.path.exists(gradle_file):
    with open(gradle_file, "r") as f:
        content = f.read()

    # Enforce stable targetSdk 34 and minSdk 24 for audioplayers and other plugins
    content = re.sub(r'(minSdk|minSdkVersion)\s*=?\s*.*', 'minSdkVersion 24', content)
    content = re.sub(r'(targetSdk|targetSdkVersion)\s*=?\s*.*', 'targetSdkVersion 34', content)
    content = re.sub(r'(compileSdk|compileSdkVersion)\s*=?\s*.*', 'compileSdk 34', content)

    with open(gradle_file, "w") as f:
        f.write(content)
    print("Successfully configured android/app/build.gradle with SDK 34 / minSdk 24")

# 4. Configure Kotlin DSL build.gradle.kts if present
kts_file = "android/app/build.gradle.kts"
if os.path.exists(kts_file):
    with open(kts_file, "r") as f:
        content = f.read()

    content = re.sub(r'(minSdk|minSdkVersion)\s*=?\s*.*', 'minSdk = 24', content)
    content = re.sub(r'(targetSdk|targetSdkVersion)\s*=?\s*.*', 'targetSdk = 34', content)
    content = re.sub(r'(compileSdk|compileSdkVersion)\s*=?\s*.*', 'compileSdk = 34', content)

    with open(kts_file, "w") as f:
        f.write(content)
    print("Successfully configured android/app/build.gradle.kts with SDK 34 / minSdk 24")
