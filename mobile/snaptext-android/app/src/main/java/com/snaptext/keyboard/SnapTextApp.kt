package com.snaptext.keyboard

import android.app.Application
import okhttp3.OkHttpClient
import java.util.concurrent.TimeUnit

class SnapTextApp : Application() {

    companion object {
        lateinit var httpClient: OkHttpClient
            private set
    }

    override fun onCreate() {
        super.onCreate()
        httpClient = OkHttpClient.Builder()
            .connectTimeout(30, TimeUnit.SECONDS)
            .readTimeout(60, TimeUnit.SECONDS)
            .writeTimeout(30, TimeUnit.SECONDS)
            .build()
    }
}
