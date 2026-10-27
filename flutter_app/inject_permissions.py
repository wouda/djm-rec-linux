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

# 4. Find pre-installed NDK if available
ndk_roots = [
    os.environ.get("ANDROID_NDK_ROOT", ""),
    os.environ.get("ANDROID_NDK_HOME", ""),
    os.environ.get("ANDROID_NDK", ""),
    os.environ.get("ANDROID_HOME", "") + "/ndk",
    os.environ.get("ANDROID_SDK_ROOT", "") + "/ndk",
    "/usr/local/lib/android/sdk/ndk",
    "/opt/android-sdk/ndk"
]
installed_ndk = None
for root in ndk_roots:
    if root and os.path.exists(root):
        if os.path.exists(os.path.join(root, "source.properties")):
            installed_ndk = os.path.basename(root.rstrip("/"))
            break
        ndks = [d for d in os.listdir(root) if os.path.isdir(os.path.join(root, d))]
        if ndks:
            installed_ndk = sorted(ndks)[-1]
            break

# If on GitHub Actions and no NDK directory found, fallback to GitHub runner pre-installed NDK r27c
if not installed_ndk and os.environ.get("GITHUB_ACTIONS"):
    installed_ndk = "27.3.13750724"

if installed_ndk:
    print(f"Detected pre-installed Android NDK: {installed_ndk}")

# 5. Configure Groovy build.gradle if present
gradle_file = "android/app/build.gradle"
if os.path.exists(gradle_file):
    with open(gradle_file, "r") as f:
        content = f.read()

    # Replace minSdkVersion with 24
    if "minSdkVersion flutter.minSdkVersion" in content:
        content = content.replace("minSdkVersion flutter.minSdkVersion", "minSdkVersion 24")
    elif "minSdkVersion" in content:
        content = re.sub(r'minSdkVersion\s+.*', 'minSdkVersion 24', content)

    # Pin ndkVersion to pre-installed version so Gradle doesn't download 1.2GB NDK
    if installed_ndk:
        if "ndkVersion" in content:
            content = re.sub(r'ndkVersion\s+.*', f'ndkVersion "{installed_ndk}"', content)
        else:
            content = re.sub(r'(android\s*\{)', f'\\1\n    ndkVersion "{installed_ndk}"', content)

    with open(gradle_file, "w") as f:
        f.write(content)
    print(f"Successfully configured android/app/build.gradle (minSdkVersion 24, ndk: {installed_ndk})")

# 6. Configure Kotlin DSL build.gradle.kts if present
kts_file = "android/app/build.gradle.kts"
if os.path.exists(kts_file):
    with open(kts_file, "r") as f:
        content = f.read()

    # In Kotlin DSL, set minSdk = 24
    if "minSdk = flutter.minSdkVersion" in content:
        content = content.replace("minSdk = flutter.minSdkVersion", "minSdk = 24")
    elif "minSdk" in content:
        content = re.sub(r'minSdk\s*=\s*.*', 'minSdk = 24', content)

    # Pin ndkVersion to pre-installed version so Gradle doesn't download 1.2GB NDK
    if installed_ndk:
        if "ndkVersion" in content:
            content = re.sub(r'ndkVersion\s*=.*', f'ndkVersion = "{installed_ndk}"', content)
        else:
            content = re.sub(r'(android\s*\{)', f'\\1\n    ndkVersion = "{installed_ndk}"', content)

    with open(kts_file, "w") as f:
        f.write(content)
    print(f"Successfully configured android/app/build.gradle.kts (minSdk = 24, ndk: {installed_ndk})")
