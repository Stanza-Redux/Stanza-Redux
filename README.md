# Stanza

This is a [Skip](https://skip.dev) dual-platform ebook reader built on top of the [Readium SDK](https://readium.org/development/readium-sdk-overview/).

<img width="240" height="507" alt="Stanza_Reader_Android" src="https://github.com/user-attachments/assets/e05784a0-69e4-4805-8ca6-a2deb490c8d3" />
<img width="244" height="492" alt="Stanza_Reader_iOS" src="https://github.com/user-attachments/assets/14c60926-1508-4a7d-b543-e957e242ae63" />
<img width="240" height="507" alt="Stanza_Library_Android" src="https://github.com/user-attachments/assets/a957654c-9c8c-403b-9de2-25be40c60dcc" />
<img width="244" height="491" alt="Stanza_Library_iOS" src="https://github.com/user-attachments/assets/9765e846-f7f6-4067-9bb7-44918600e508" />

<img width="240" height="507" alt="Stanza_Browse_Android" src="https://github.com/user-attachments/assets/691e249a-d4ed-41c7-abb1-a4ec66a59c99" />
<img width="244" height="492" alt="Stanza_Browse_iOS" src="https://github.com/user-attachments/assets/0450099f-78a2-4a9a-9268-c2e8bb58f807" />
<img width="240" height="507" alt="Stanza_Settings_Android" src="https://github.com/user-attachments/assets/b8735a4b-e219-47b6-9187-29608bf0476c" />
<img width="244" height="494" alt="Stanza_Settings_iOS" src="https://github.com/user-attachments/assets/4bc6a3a1-aadc-4184-ba06-6faa700bda8c" />


## Building

This project is both a stand-alone Swift Package Manager module,
as well as an Xcode project that builds and transpiles the project
into a Kotlin Gradle project for Android using the Skip plugin.

Building the module requires that Skip be installed using
[Homebrew](https://brew.sh) with `brew install skiptools/skip/skip`.

This will also install the necessary transpiler prerequisites:
Kotlin, Gradle, and the Android build tools.

Installation prerequisites can be confirmed by running `skip checkup`.

## Testing

The module can be tested using the standard `swift test` command
or by running the test target for the macOS destination in Xcode,
which will run the Swift tests as well as the transpiled
Kotlin JUnit tests in the Robolectric Android simulation environment.

Parity testing can be performed with `skip test`,
which will output a table of the test results for both platforms.

## Running

Xcode and Android Studio must be downloaded and installed in order to
run the app in the iOS simulator / Android emulator.
An Android emulator must already be running, which can be launched from 
Android Studio's Device Manager.

To run both the Swift and Kotlin apps simultaneously, 
launch the StanzaApp target from Xcode.
A build phases runs the "Launch Android APK" script that
will deploy the transpiled app a running Android emulator or connected device.
Logging output for the iOS app can be viewed in the Xcode console, and in
Android Studio's logcat tab for the transpiled Kotlin app.
