package com.doraemon

import android.graphics.Color
import android.os.Build
import android.os.Bundle
import android.view.View
import android.view.WindowInsetsController
import androidx.appcompat.app.AppCompatActivity
import android.webkit.WebView
import android.webkit.WebViewClient
import android.webkit.WebSettings
import android.content.res.Configuration

class MainActivity : AppCompatActivity() {

    private lateinit var webView: WebView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        webView = findViewById(R.id.webView)
        setupWebView()
        loadUrl()

        // 设置状态栏和导航栏颜色跟随系统主题
        setupSystemBars()

        // 设置根布局背景色
        setupRootBackground()
    }

    private fun setupWebView() {
        val settings = webView.settings
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.databaseEnabled = true
        settings.loadWithOverviewMode = true
        settings.useWideViewPort = true
        settings.setSupportZoom(true)
        settings.builtInZoomControls = true
        settings.displayZoomControls = false
        
        // 允许加载本地文件
        settings.allowFileAccess = true
        settings.allowContentAccess = true
        settings.allowFileAccessFromFileURLs = true
        settings.allowUniversalAccessFromFileURLs = true
        
        // 设置混合内容模式
        if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.LOLLIPOP) {
            settings.mixedContentMode = android.webkit.WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
        }

        // 设置WebView背景色跟随系统主题
        val isDarkTheme = resources.configuration.uiMode and
                android.content.res.Configuration.UI_MODE_NIGHT_MASK ==
                android.content.res.Configuration.UI_MODE_NIGHT_YES

        if (isDarkTheme) {
            webView.setBackgroundColor(Color.BLACK)
        } else {
            webView.setBackgroundColor(Color.WHITE)
        }

        // 添加JavaScript接口
        webView.addJavascriptInterface(object {
            @android.webkit.JavascriptInterface
            fun isWebView(): String {
                return "true"
            }

            @android.webkit.JavascriptInterface
            fun savePanelAddress(name: String, address: String) {
                val sharedPrefs =
                    getSharedPreferences("panel_config", android.content.Context.MODE_PRIVATE)
                val editor = sharedPrefs.edit()

                // 获取现有的地址列表
                val existingAddresses =
                    sharedPrefs.getStringSet("panel_addresses", mutableSetOf()) ?: mutableSetOf()
                val newAddresses = existingAddresses.toMutableSet()

                // 添加新地址（格式：name|address）
                newAddresses.add("$name|$address")

                editor.putStringSet("panel_addresses", newAddresses)
                editor.apply()
            }

            @android.webkit.JavascriptInterface
            fun getPanelAddresses(): String {
                val sharedPrefs =
                    getSharedPreferences("panel_config", android.content.Context.MODE_PRIVATE)
                val addresses =
                    sharedPrefs.getStringSet("panel_addresses", mutableSetOf()) ?: mutableSetOf()
                return if (addresses.isEmpty()) "" else addresses.joinToString(",")
            }

            @android.webkit.JavascriptInterface
            fun setCurrentPanelAddress(address: String) {
                val sharedPrefs =
                    getSharedPreferences("panel_config", android.content.Context.MODE_PRIVATE)
                val editor = sharedPrefs.edit()
                editor.putString("current_panel_address", address)
                editor.apply()
            }

            @android.webkit.JavascriptInterface
            fun getCurrentPanelAddress(): String {
                val sharedPrefs =
                    getSharedPreferences("panel_config", android.content.Context.MODE_PRIVATE)
                return sharedPrefs.getString("current_panel_address", "") ?: ""
            }

            @android.webkit.JavascriptInterface
            fun deletePanelAddress(name: String, address: String) {
                val sharedPrefs =
                    getSharedPreferences("panel_config", android.content.Context.MODE_PRIVATE)
                val editor = sharedPrefs.edit()

                val existingAddresses =
                    sharedPrefs.getStringSet("panel_addresses", mutableSetOf()) ?: mutableSetOf()
                val newAddresses = existingAddresses.toMutableSet()

                // 删除指定地址
                newAddresses.remove("$name|$address")

                // 如果删除的是当前选中的地址，清除当前地址
                val currentAddress = sharedPrefs.getString("current_panel_address", "") ?: ""
                if (currentAddress == address) {
                    editor.remove("current_panel_address")
                }

                editor.putStringSet("panel_addresses", newAddresses)
                editor.apply()
            }
        }, "AndroidInterface")

        webView.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(view: WebView?, url: String?): Boolean {
                url?.let { view?.loadUrl(it) }
                return true
            }
        }
    }

    private fun loadUrl() {
        webView.getSettings().setJavaScriptEnabled(true);
        webView.getSettings().setAllowFileAccess(true); // 允许访问 assets
        webView.getSettings().setAllowFileAccessFromFileURLs(true); // Android 4.1+
        webView.getSettings().setAllowUniversalAccessFromFileURLs(true); // Android 4.1+

        webView.loadUrl("file:///android_asset/index.html")
    }

    private fun setupSystemBars() {
        // 检测当前是否为暗色主题
        val isDarkTheme = resources.configuration.uiMode and
                android.content.res.Configuration.UI_MODE_NIGHT_MASK ==
                android.content.res.Configuration.UI_MODE_NIGHT_YES

        if (isDarkTheme) {
            // 暗色主题：深色状态栏，浅色文字
            window.statusBarColor = Color.BLACK
            window.navigationBarColor = Color.BLACK

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                window.insetsController?.setSystemBarsAppearance(
                    0, // 清除浅色状态栏标志
                    WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                            WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
                )
            } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                @Suppress("DEPRECATION")
                window.decorView.systemUiVisibility =
                    window.decorView.systemUiVisibility and
                            View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR.inv()
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    @Suppress("DEPRECATION")
                    window.decorView.systemUiVisibility =
                        window.decorView.systemUiVisibility and
                                View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR.inv()
                }
            }
        } else {
            // 浅色主题：浅色状态栏，深色文字
            window.statusBarColor = Color.WHITE
            window.navigationBarColor = Color.WHITE

            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                window.insetsController?.setSystemBarsAppearance(
                    WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                            WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS,
                    WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                            WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
                )
            } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                @Suppress("DEPRECATION")
                window.decorView.systemUiVisibility =
                    window.decorView.systemUiVisibility or View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    @Suppress("DEPRECATION")
                    window.decorView.systemUiVisibility =
                        window.decorView.systemUiVisibility or View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR
                }
            }
        }
    }

    override fun onBackPressed() {
        if (webView.canGoBack()) webView.goBack() else super.onBackPressed()
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        // 当系统主题变化时，重新设置状态栏、WebView背景色和根布局背景色
        setupSystemBars()
        setupWebViewBackground()
        setupRootBackground()
    }

    private fun setupWebViewBackground() {
        val isDarkTheme = resources.configuration.uiMode and
                android.content.res.Configuration.UI_MODE_NIGHT_MASK ==
                android.content.res.Configuration.UI_MODE_NIGHT_YES

        if (isDarkTheme) {
            webView.setBackgroundColor(Color.BLACK)
        } else {
            webView.setBackgroundColor(Color.WHITE)
        }
    }

    private fun setupRootBackground() {
        val isDarkTheme = resources.configuration.uiMode and
                android.content.res.Configuration.UI_MODE_NIGHT_MASK ==
                android.content.res.Configuration.UI_MODE_NIGHT_YES

        val rootView = findViewById<android.view.View>(android.R.id.content)
        if (isDarkTheme) {
            rootView.setBackgroundColor(Color.BLACK)
        } else {
            rootView.setBackgroundColor(Color.WHITE)
        }
    }
}
