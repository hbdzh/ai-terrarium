package com.aiterrarium.desktop

import android.graphics.Color
import android.os.Bundle
import android.view.View
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    appWebView = webView
    webView.overScrollMode = View.OVER_SCROLL_NEVER
    webView.settings.textZoom = 100
    webView.isVerticalScrollBarEnabled = false
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge(
      statusBarStyle = SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT),
      navigationBarStyle = SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT)
    )
    super.onCreate(savedInstanceState)

    val controller = WindowInsetsControllerCompat(window, window.decorView)
    controller.isAppearanceLightStatusBars = true
    controller.isAppearanceLightNavigationBars = true

    // 确保状态栏、刘海屏、底部导航手势栏与软键盘(IME)均安全留白，防止内容被遮挡
    val rootView = findViewById<View>(android.R.id.content)
    rootView?.setBackgroundColor(Color.parseColor("#F6F6F8"))
    rootView?.let { view ->
      ViewCompat.setOnApplyWindowInsetsListener(view) { v, windowInsets ->
        val insets = windowInsets.getInsets(
          WindowInsetsCompat.Type.systemBars() or
            WindowInsetsCompat.Type.displayCutout() or
            WindowInsetsCompat.Type.ime()
        )
        v.setPadding(insets.left, insets.top, insets.right, insets.bottom)
        WindowInsetsCompat.CONSUMED
      }
    }

    // 拦截系统返回手势/按键，优先关闭前端抽屉/弹窗/回放模式
    val backCallback = object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val wv = appWebView
        if (wv == null) {
          isEnabled = false
          onBackPressedDispatcher.onBackPressed()
          isEnabled = true
          return
        }
        wv.evaluateJavascript(
          "(function(){ return typeof window.__handleAndroidBack === 'function' ? !!window.__handleAndroidBack() : false; })()"
        ) { result ->
          if (result != "true") {
            isEnabled = false
            onBackPressedDispatcher.onBackPressed()
            isEnabled = true
          }
        }
      }
    }
    onBackPressedDispatcher.addCallback(this, backCallback)
  }
}
