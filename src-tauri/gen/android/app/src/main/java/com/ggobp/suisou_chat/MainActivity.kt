package com.ggobp.suisou_chat

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat

class MainActivity : TauriActivity() {
  private external fun initializeApiKeyStore(context: android.content.Context)

  override fun onCreate(savedInstanceState: Bundle?) {
    Rust.hashCode()
    initializeApiKeyStore(applicationContext)
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    requestResearchNotificationPermission()
  }

  private fun requestResearchNotificationPermission() {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) {
      return
    }
    if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) ==
      PackageManager.PERMISSION_GRANTED
    ) {
      return
    }
    val preferences = getSharedPreferences("suisou_permissions", MODE_PRIVATE)
    if (preferences.getBoolean("notification_permission_requested", false)) {
      return
    }
    preferences.edit().putBoolean("notification_permission_requested", true).apply()
    ActivityCompat.requestPermissions(
      this,
      arrayOf(Manifest.permission.POST_NOTIFICATIONS),
      RESEARCH_NOTIFICATION_PERMISSION_REQUEST
    )
  }

  companion object {
    private const val RESEARCH_NOTIFICATION_PERMISSION_REQUEST = 2401
  }
}
