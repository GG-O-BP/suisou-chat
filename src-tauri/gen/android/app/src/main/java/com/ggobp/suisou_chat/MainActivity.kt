package com.ggobp.suisou_chat

import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  private external fun initializeApiKeyStore(context: android.content.Context)

  override fun onCreate(savedInstanceState: Bundle?) {
    Rust.hashCode()
    initializeApiKeyStore(applicationContext)
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }
}
