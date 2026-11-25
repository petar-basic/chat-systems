package com.chatsystems.huddle

import android.app.Activity
import android.content.Intent
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.Plugin

@InvokeArg
class BeginArgs {
    lateinit var channel: String
    lateinit var title: String
    lateinit var text: String
}

@TauriPlugin
class HuddlePlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun begin(invoke: Invoke) {
        val args = invoke.parseArgs(BeginArgs::class.java)
        val intent = Intent(activity, HuddleService::class.java)
            .putExtra(HuddleService.CHANNEL_NAME, args.channel)
            .putExtra(HuddleService.TITLE, args.title)
            .putExtra(HuddleService.TEXT, args.text)
        try {
            ContextCompat.startForegroundService(activity, intent)
            invoke.resolve()
        } catch (error: Exception) {
            invoke.reject(error.message ?: error.javaClass.simpleName)
        }
    }

    @Command
    fun end(invoke: Invoke) {
        activity.stopService(Intent(activity, HuddleService::class.java))
        invoke.resolve()
    }
}
