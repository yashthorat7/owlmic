# Preserve JNI methods and native bridge classes
-keepclasseswithmembernames class * {
    native <methods>;
}

-keep class com.owlmic.media.audio.Oboe { *; }
-keep class com.owlmic.media.codec.OpusEncoder { *; }
-keep class com.owlmic.media.codec.OpusEncoder$* { *; }
-keep class com.owlmic.media.codec.OpusDecoder { *; }
-keep class com.owlmic.media.codec.OpusDecoder$* { *; }
