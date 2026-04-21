# OkHttp
-dontwarn okhttp3.**
-keep class okhttp3.** { *; }

# Coroutines
-keepnames class kotlinx.coroutines.** { *; }

# Keep IME service
-keep class com.snaptext.keyboard.SnapTextIME { *; }
