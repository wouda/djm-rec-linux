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
    print("Successfully configured AndroidManifest.xml")

# 2. Configure android/local.properties to set SDK baselines
props_file = "android/local.properties"
props_lines = []
if os.path.exists(props_file):
    with open(props_file, "r") as f:
        props_lines = f.readlines()

props_lines = [
    line for line in props_lines
    if not any(k in line for k in ["flutter.minSdkVersion"])
]
props_lines.append("flutter.minSdkVersion=24\n")

with open(props_file, "w") as f:
    f.writelines(props_lines)
print("Successfully set flutter.minSdkVersion=24 in android/local.properties")

# 3. Configure android/gradle.properties
gradle_props = "android/gradle.properties"
props_lines = []
if os.path.exists(gradle_props):
    with open(gradle_props, "r") as f:
        props_lines = f.readlines()

props_lines = [
    line for line in props_lines
    if not any(k in line for k in ["kotlin.jvm.target.validation.mode"])
]
props_lines.append("\n# Build compatibility flags\n")
props_lines.append("kotlin.jvm.target.validation.mode=IGNORE\n")

with open(gradle_props, "w") as f:
    f.writelines(props_lines)
print("Successfully configured android/gradle.properties")

# 4. Configure Groovy build.gradle if present
gradle_file = "android/app/build.gradle"
if os.path.exists(gradle_file):
    with open(gradle_file, "r") as f:
        content = f.read()

    # Replace minSdkVersion with 24
    if "minSdkVersion flutter.minSdkVersion" in content:
        content = content.replace("minSdkVersion flutter.minSdkVersion", "minSdkVersion 24")
    elif "minSdkVersion" in content:
        content = re.sub(r'minSdkVersion\s+.*', 'minSdkVersion 24', content)

    # Remove ndkVersion so Gradle uses pre-installed runner NDK instead of downloading 1.2GB NDK
    content = re.sub(r'ndkVersion\s+.*', '', content)

    with open(gradle_file, "w") as f:
        f.write(content)
    print("Successfully configured android/app/build.gradle with minSdkVersion 24")

# 5. Configure Kotlin DSL build.gradle.kts if present
kts_file = "android/app/build.gradle.kts"
if os.path.exists(kts_file):
    with open(kts_file, "r") as f:
        content = f.read()

    # In Kotlin DSL, set minSdk = 24
    if "minSdk = flutter.minSdkVersion" in content:
        content = content.replace("minSdk = flutter.minSdkVersion", "minSdk = 24")
    elif "minSdk" in content:
        content = re.sub(r'minSdk\s*=\s*flutter\.minSdkVersion', 'minSdk = 24', content)

    # Remove ndkVersion so Gradle uses pre-installed runner NDK instead of downloading 1.2GB NDK
    content = re.sub(r'ndkVersion\s*=.*', '', content)

    with open(kts_file, "w") as f:
        f.write(content)
    print("Successfully configured android/app/build.gradle.kts with minSdk = 24")
