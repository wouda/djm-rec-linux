import os

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

# 2. Configure android/local.properties to set SDK baselines safely without editing build.gradle.kts
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
    if not any(k in line for k in ["kotlin.jvm.target.validation.mode", "android.useAndroidX", "android.enableJetifier"])
]
props_lines.append("\n# Build compatibility flags\n")
props_lines.append("kotlin.jvm.target.validation.mode=IGNORE\n")
props_lines.append("android.useAndroidX=true\n")
props_lines.append("android.enableJetifier=true\n")

with open(gradle_props, "w") as f:
    f.writelines(props_lines)
print("Successfully configured android/gradle.properties")

# 4. Proguard rules for audioplayers and media3
proguard_file = "android/app/proguard-rules.pro"
rules = """
-dontwarn xyz.ggl.audioplayers.**
-keep class xyz.ggl.audioplayers.** { *; }
-dontwarn com.audioplayers.**
-keep class com.audioplayers.** { *; }
-dontwarn androidx.media3.**
-keep class androidx.media3.** { *; }
"""
with open(proguard_file, "w") as f:
    f.write(rules)
print("Successfully created android/app/proguard-rules.pro")
