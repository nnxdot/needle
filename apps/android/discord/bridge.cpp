// A test only: Discord status from the phone, through Discord's Social SDK (C++ only).
// Kotlin calls these from one thread (DiscordTest.kt). Built by scripts/android-discord.sh.
#define DISCORDPP_IMPLEMENTATION
#include "discordpp.h"

#include <android/log.h>
#include <jni.h>

#include <memory>
#include <string>

#define LOG(...) __android_log_print(ANDROID_LOG_INFO, "NeedleDiscord", __VA_ARGS__)

static std::shared_ptr<discordpp::Client> client;

static std::string text(JNIEnv* env, jstring s) {
    const char* chars = env->GetStringUTFChars(s, nullptr);
    std::string out(chars);
    env->ReleaseStringUTFChars(s, chars);
    return out;
}

extern "C" JNIEXPORT void JNICALL
Java_fyi_nnx_needle_DiscordTest_nativeStart(JNIEnv*, jobject, jlong appId) {
    client = std::make_shared<discordpp::Client>();
    client->AddLogCallback(
        [](std::string message, discordpp::LoggingSeverity) { LOG("sdk: %s", message.c_str()); },
        discordpp::LoggingSeverity::Info);
    client->SetApplicationId(static_cast<uint64_t>(appId));
    LOG("started");
}

extern "C" JNIEXPORT void JNICALL
Java_fyi_nnx_needle_DiscordTest_nativeShow(JNIEnv* env, jobject, jstring title, jstring artist,
                                           jstring album, jlong started) {
    if (!client) return;
    discordpp::Activity activity;
    activity.SetType(discordpp::ActivityTypes::Listening);
    activity.SetName("Needle");
    activity.SetStatusDisplayType(discordpp::StatusDisplayTypes::Name);
    activity.SetDetails(text(env, title));
    activity.SetState(text(env, artist));
    discordpp::ActivityAssets assets;
    assets.SetLargeImage("needle");
    assets.SetLargeText(text(env, album));
    activity.SetAssets(assets);
    discordpp::ActivityTimestamps times;
    times.SetStart(static_cast<uint64_t>(started));
    activity.SetTimestamps(times);
    client->UpdateRichPresence(activity, [](discordpp::ClientResult result) {
        if (result.Successful()) {
            LOG("shown");
        } else {
            LOG("refused: %s", result.Error().c_str());
        }
    });
}

extern "C" JNIEXPORT void JNICALL
Java_fyi_nnx_needle_DiscordTest_nativeRunCallbacks(JNIEnv*, jobject) {
    discordpp::RunCallbacks();
}
