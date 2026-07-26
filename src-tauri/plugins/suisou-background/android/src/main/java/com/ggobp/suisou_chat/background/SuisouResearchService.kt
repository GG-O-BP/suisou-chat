package com.ggobp.suisou_chat.background

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat

class SuisouResearchService : Service() {
    companion object {
        const val ACTION_START = "com.ggobp.suisou_chat.background.START"
        const val ACTION_UPDATE = "com.ggobp.suisou_chat.background.UPDATE"
        const val ACTION_STOP = "com.ggobp.suisou_chat.background.STOP"
        const val ACTION_CANCEL = "com.ggobp.suisou_chat.background.CANCEL"
        const val EXTRA_REQUEST_ID = "request_id"
        const val EXTRA_STAGE = "stage"
        const val EXTRA_STATUS = "status"
        const val EXTRA_MODE = "mode"
        const val EXTRA_HAS_OUTPUT = "has_output"

        private const val CHANNEL_ID = "suisou_research"
        private const val NOTIFICATION_ID = 2401
    }

    private var requestId: String? = null
    private var stage = "connecting"
    private var status = "running"
    private var mode = "search"
    private var hasOutput = false

    override fun onCreate() {
        super.onCreate()
        ensureChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_STOP -> {
                stopForeground(STOP_FOREGROUND_REMOVE)
                stopSelf()
                return START_NOT_STICKY
            }
            ACTION_CANCEL -> {
                intent.getStringExtra(EXTRA_REQUEST_ID)?.let { request ->
                    cancelResearch(request)
                }
                return START_NOT_STICKY
            }
            ACTION_START, ACTION_UPDATE, null -> {
                requestId = intent?.getStringExtra(EXTRA_REQUEST_ID) ?: requestId
                stage = intent?.getStringExtra(EXTRA_STAGE) ?: stage
                status = intent?.getStringExtra(EXTRA_STATUS) ?: status
                mode = intent?.getStringExtra(EXTRA_MODE) ?: mode
                hasOutput = intent?.getBooleanExtra(EXTRA_HAS_OUTPUT, hasOutput) ?: hasOutput
                showNotification()
            }
        }
        return START_NOT_STICKY
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onTimeout(startId: Int, fgsType: Int) {
        requestId?.let { cancelResearch(it) }
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf(startId)
    }

    private fun showNotification() {
        val request = requestId ?: return
        val openIntent = packageManager.getLaunchIntentForPackage(packageName)?.apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val openPendingIntent = openIntent?.let {
            PendingIntent.getActivity(
                this,
                0,
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
            )
        }
        val cancelIntent = Intent(this, SuisouResearchService::class.java).apply {
            action = ACTION_CANCEL
            putExtra(EXTRA_REQUEST_ID, request)
        }
        val cancelPendingIntent = PendingIntent.getService(
            this,
            1,
            cancelIntent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE
        )
        val notification = NotificationCompat.Builder(this, CHANNEL_ID)
            .setSmallIcon(applicationInfo.icon)
            .setContentTitle("Suisou 연구 잠수")
            .setContentText(stageLabel(stage, mode, hasOutput))
            .setOnlyAlertOnce(true)
            .setOngoing(status == "running")
            .setProgress(0, 0, status == "running")
            .addAction(0, "중단", cancelPendingIntent)
            .setCategory(NotificationCompat.CATEGORY_PROGRESS)
            .apply {
                openPendingIntent?.let(::setContentIntent)
            }
            .build()

        ServiceCompat.startForeground(
            this,
            NOTIFICATION_ID,
            notification,
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC
            } else {
                0
            }
        )
    }

    private fun ensureChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return
        val manager = getSystemService(NotificationManager::class.java)
        val channel = NotificationChannel(
            CHANNEL_ID,
            "백그라운드 연구",
            NotificationManager.IMPORTANCE_LOW
        ).apply {
            description = "앱을 벗어난 동안 진행되는 Suisou 연구 상태"
            setShowBadge(false)
        }
        manager.createNotificationChannel(channel)
    }

    private external fun cancelResearch(requestId: String): Boolean
}

private fun stageLabel(stage: String, mode: String, hasOutput: Boolean): String {
    if (hasOutput) {
        return if (mode == "create") "창작물을 쓰는 중" else "발견한 내용을 비추는 중"
    }
    return when (stage) {
        "connecting" -> "Sakana에 연결 중"
        "searching" -> "웹 자료를 탐색하는 중"
        "creating" -> "아이디어를 빚는 중"
        "reasoning" -> if (mode == "create") "구성을 다듬는 중" else "출처를 비교하는 중"
        "writing" -> if (mode == "create") "창작물을 쓰는 중" else "답변을 작성하는 중"
        else -> "연구를 계속하는 중"
    }
}
