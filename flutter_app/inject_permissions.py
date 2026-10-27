import os
import re
import glob

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
    if not any(k in line for k in ["kotlin.jvm.target.validation.mode", "android.useAndroidX", "android.enableJetifier"])
]
props_lines.append("\n# Build compatibility flags\n")
props_lines.append("kotlin.jvm.target.validation.mode=IGNORE\n")
props_lines.append("android.useAndroidX=true\n")
props_lines.append("android.enableJetifier=true\n")

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

if installed_ndk:
    print(f"Detected pre-installed Android NDK: {installed_ndk}")

# 5. Configure Groovy root android/build.gradle if present
root_gradle = "android/build.gradle"
if os.path.exists(root_gradle):
    with open(root_gradle, "r") as f:
        content = f.read()

    subprojects_block = """
subprojects {
    afterEvaluate { project ->
        if (project.hasProperty("android")) {
            project.android {
                compileOptions {
                    sourceCompatibility JavaVersion.VERSION_17
                    targetCompatibility JavaVersion.VERSION_17
                }
            }
        }
        tasks.withType(org.jetbrains.kotlin.gradle.tasks.KotlinCompile).configureEach {
            kotlinOptions {
                jvmTarget = "17"
            }
        }
    }
}
"""
    if "subprojects {" not in content:
        content += "\n" + subprojects_block
        with open(root_gradle, "w") as f:
            f.write(content)
        print("Successfully added Java 17 subprojects alignment to android/build.gradle")

# 6. Configure Groovy android/app/build.gradle if present
gradle_file = "android/app/build.gradle"
if os.path.exists(gradle_file):
    with open(gradle_file, "r") as f:
        content = f.read()

    # Replace minSdkVersion with 24
    if "minSdkVersion flutter.minSdkVersion" in content:
        content = content.replace("minSdkVersion flutter.minSdkVersion", "minSdkVersion 24")
    elif "minSdkVersion" in content:
        content = re.sub(r'minSdkVersion\s+.*', 'minSdkVersion 24', content)

    # Align compileOptions and kotlinOptions to Java 17
    content = re.sub(r'sourceCompatibility\s+JavaVersion\.\w+', 'sourceCompatibility JavaVersion.VERSION_17', content)
    content = re.sub(r'targetCompatibility\s+JavaVersion\.\w+', 'targetCompatibility JavaVersion.VERSION_17', content)
    content = re.sub(r'jvmTarget\s*=\s*[\'"][^\'"]+[\'"]', "jvmTarget = '17'", content)

    # Pin ndkVersion if pre-installed
    if installed_ndk:
        if "ndkVersion" in content:
            content = re.sub(r'ndkVersion\s+.*', f'ndkVersion "{installed_ndk}"', content)
        else:
            content = re.sub(r'(android\s*\{)', f'\\1\n    ndkVersion "{installed_ndk}"', content)

    with open(gradle_file, "w") as f:
        f.write(content)
    print(f"Successfully configured android/app/build.gradle (minSdkVersion 24, Java 17)")

# 7. Configure Kotlin DSL android/app/build.gradle.kts if present
kts_file = "android/app/build.gradle.kts"
if os.path.exists(kts_file):
    with open(kts_file, "r") as f:
        content = f.read()

    # In Kotlin DSL, set minSdk = 24
    if "minSdk = flutter.minSdkVersion" in content:
        content = content.replace("minSdk = flutter.minSdkVersion", "minSdk = 24")
    elif "minSdk" in content:
        content = re.sub(r'minSdk\s*=\s*.*', 'minSdk = 24', content)

    # Align compileOptions and kotlinOptions to Java 17
    content = re.sub(r'sourceCompatibility\s*=\s*JavaVersion\.\w+', 'sourceCompatibility = JavaVersion.VERSION_17', content)
    content = re.sub(r'targetCompatibility\s*=\s*JavaVersion\.\w+', 'targetCompatibility = JavaVersion.VERSION_17', content)
    content = re.sub(r'jvmTarget\s*=\s*.*', 'jvmTarget = "17"', content)

    # Pin ndkVersion if pre-installed
    if installed_ndk:
        if "ndkVersion" in content:
            content = re.sub(r'ndkVersion\s*=.*', f'ndkVersion = "{installed_ndk}"', content)
        else:
            content = re.sub(r'(android\s*\{)', f'\\1\n    ndkVersion = "{installed_ndk}"', content)

    with open(kts_file, "w") as f:
        f.write(content)
    print(f"Successfully configured android/app/build.gradle.kts (minSdk = 24, Java 17)")

# 8. Patch plugin build.gradle in pub cache (e.g. audioplayers_android)
cache_patterns = [
    os.path.expanduser("~/.pub-cache/hosted/pub.dev/audioplayers_android-*/android/build.gradle"),
    os.path.expanduser("~/.pub-cache/hosted/pub.dartlang.org/audioplayers_android-*/android/build.gradle"),
]
for pattern in cache_patterns:
    for plugin_gradle in glob.glob(pattern):
        try:
            with open(plugin_gradle, "r") as f:
                p_content = f.read()
            original = p_content
            # Align Java & Kotlin to Java 17
            p_content = re.sub(r'sourceCompatibility\s+JavaVersion\.\w+', 'sourceCompatibility JavaVersion.VERSION_17', p_content)
            p_content = re.sub(r'targetCompatibility\s+JavaVersion\.\w+', 'targetCompatibility JavaVersion.VERSION_17', p_content)
            p_content = re.sub(r'jvmTarget\s*=\s*[\'"][^\'"]+[\'"]', "jvmTarget = '17'", p_content)
            if p_content != original:
                with open(plugin_gradle, "w") as f:
                    f.write(p_content)
                print(f"Successfully patched {plugin_gradle} for Java 17 compatibility")
        except Exception as e:
            print(f"Notice: could not patch {plugin_gradle}: {e}")
