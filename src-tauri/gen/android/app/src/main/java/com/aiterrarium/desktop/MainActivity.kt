package com.aiterrarium.desktop

import android.content.res.Configuration
import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.view.HapticFeedbackConstants
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null
  private var safeTopDp = 0f
  private var safeRightDp = 0f
  private var safeBottomDp = 0f
  private var safeLeftDp = 0f
  private var imeBottomDp = 0f

  private fun buildInsetsJson(): String {
    return "{\"top\":$safeTopDp,\"right\":$safeRightDp,\"bottom\":$safeBottomDp,\"left\":$safeLeftDp,\"imeBottom\":$imeBottomDp}"
  }

  private fun pushInsetsToWebView(webView: WebView?) {
    val wv = webView ?: return
    val imeOpen = imeBottomDp > 1f
    val js = """
      (function(){
        var r = document.documentElement;
        if(!r) return;
        r.style.setProperty('--safe-top', '${safeTopDp}px');
        r.style.setProperty('--safe-right', '${safeRightDp}px');
        r.style.setProperty('--safe-bottom', '${safeBottomDp}px');
        r.style.setProperty('--safe-left', '${safeLeftDp}px');
        r.style.setProperty('--ime-bottom', '${imeBottomDp}px');
        r.classList.toggle('ime-open', $imeOpen);
      })();
    """.trimIndent()
    wv.post { wv.evaluateJavascript(js, null) }
  }

  inner class AndroidNativeBridge(private val webView: WebView) {
    @JavascriptInterface
    fun haptic(kind: String) {
      val code = when (kind) {
        "confirm", "success" ->
          if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            HapticFeedbackConstants.CONFIRM
          } else {
            HapticFeedbackConstants.VIRTUAL_KEY
          }
        "error", "reject" ->
          if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            HapticFeedbackConstants.REJECT
          } else {
            HapticFeedbackConstants.LONG_PRESS
          }
        "medium", "impact" -> HapticFeedbackConstants.KEYBOARD_TAP
        else ->
          if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            HapticFeedbackConstants.CONTEXT_CLICK
          } else {
            HapticFeedbackConstants.CLOCK_TICK
          }
      }
      webView.post { webView.performHapticFeedback(code) }
    }

    @JavascriptInterface
    fun getInsetsJson(): String = buildInsetsJson()

    @JavascriptInterface
    fun syncInsets() {
      pushInsetsToWebView(webView)
    }

    @JavascriptInterface
    fun setStatusBarLightIcons(lightIcons: Boolean) {
      runOnUiThread {
        val controller = WindowInsetsControllerCompat(window, window.decorView)
        controller.isAppearanceLightStatusBars = lightIcons
        controller.isAppearanceLightNavigationBars = lightIcons
        val bgColor = if (lightIcons) Color.WHITE else Color.parseColor("#18181B")
        findViewById<View>(android.R.id.content)?.setBackgroundColor(bgColor)
      }
    }
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    appWebView = webView
    webView.overScrollMode = View.OVER_SCROLL_NEVER
    webView.settings.textZoom = 100
    webView.isVerticalScrollBarEnabled = false
    webView.addJavascriptInterface(AndroidNativeBridge(webView), "AndroidNative")
    pushInsetsToWebView(webView)
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    val nightMode = resources.configuration.uiMode and Configuration.UI_MODE_NIGHT_MASK
    val isNight = nightMode == Configuration.UI_MODE_NIGHT_YES
    enableEdgeToEdge(
      statusBarStyle = if (isNight) {
        SystemBarStyle.dark(Color.TRANSPARENT)
      } else {
        SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT)
      },
      navigationBarStyle = if (isNight) {
        SystemBarStyle.dark(Color.TRANSPARENT)
      } else {
        SystemBarStyle.light(Color.TRANSPARENT, Color.TRANSPARENT)
      }
    )
    super.onCreate(savedInstanceState)

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      window.isNavigationBarContrastEnforced = false
      window.isStatusBarContrastEnforced = false
    }

    val controller = WindowInsetsControllerCompat(window, window.decorView)
    controller.isAppearanceLightStatusBars = !isNight
    controller.isAppearanceLightNavigationBars = !isNight

    // True Edge-to-Edge: WebView draws behind status bar & gesture bar;
    // only pad rootView for the soft keyboard (IME) while pushing safe insets as CSS vars.
    val rootView = findViewById<View>(android.R.id.content)
    rootView?.setBackgroundColor(if (isNight) Color.parseColor("#18181B") else Color.WHITE)
    rootView?.let { view ->
      ViewCompat.setOnApplyWindowInsetsListener(view) { v, windowInsets ->
        val sys = windowInsets.getInsets(
          WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
        )
        val ime = windowInsets.getInsets(WindowInsetsCompat.Type.ime())
        val density = resources.displayMetrics.density.coerceAtLeast(1f)

        val newTop = sys.top / density
        val newRight = sys.right / density
        val newBottom = sys.bottom / density
        val newLeft = sys.left / density

        val imeOnlyPx = (ime.bottom - sys.bottom).coerceAtLeast(0)
        val newImeBottom = imeOnlyPx / density

        val changed =
          kotlin.math.abs(newTop - safeTopDp) > 0.2f ||
          kotlin.math.abs(newRight - safeRightDp) > 0.2f ||
          kotlin.math.abs(newBottom - safeBottomDp) > 0.2f ||
          kotlin.math.abs(newLeft - safeLeftDp) > 0.2f ||
          (imeBottomDp > 1f) != (newImeBottom > 1f)

        safeTopDp = newTop
        safeRightDp = newRight
        safeBottomDp = newBottom
        safeLeftDp = newLeft
        imeBottomDp = newImeBottom

        if (v.paddingBottom != imeOnlyPx) {
          v.setPadding(0, 0, 0, imeOnlyPx)
        }
        if (changed) {
          pushInsetsToWebView(appWebView)
        }
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
