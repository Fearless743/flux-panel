package com.doraemon

import android.graphics.Color
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.View
import android.view.WindowInsetsController
import androidx.appcompat.app.AppCompatActivity
import android.webkit.*
import androidx.webkit.WebViewAssetLoader
import android.content.res.Configuration

class MainActivity : AppCompatActivity() {

    private lateinit var webView: WebView
    private lateinit var assetLoader: WebViewAssetLoader

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        webView = findViewById(R.id.webView)
        WebView.setWebContentsDebuggingEnabled(true)
        setupAssetLoader()
        setupWebView()
        loadUrl()

        setupSystemBars()
        setupRootBackground()
    }

    private fun setupAssetLoader() {
        assetLoader = WebViewAssetLoader.Builder()
            .addPathHandler("/assets/", WebViewAssetLoader.AssetsPathHandler(this))
            .build()
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

        // 隐藏滚动条但不影响滚动
        webView.isVerticalScrollBarEnabled = false
        webView.isHorizontalScrollBarEnabled = false
        webView.scrollBarStyle = View.SCROLLBARS_INSIDE_OVERLAY

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.LOLLIPOP) {
            settings.mixedContentMode = WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
        }

        val isDarkTheme = resources.configuration.uiMode and
                Configuration.UI_MODE_NIGHT_MASK ==
                Configuration.UI_MODE_NIGHT_YES

        webView.setBackgroundColor(if (isDarkTheme) Color.BLACK else Color.WHITE)


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
            override fun shouldInterceptRequest(
                view: WebView?,
                request: WebResourceRequest?
            ): WebResourceResponse? {
                val response = request?.url?.let { assetLoader.shouldInterceptRequest(it) }
                // 👉 fallback: 如果没有匹配静态文件，返回 index.html
                return response ?: if (request?.url?.path?.startsWith("/assets/") == true) {
                    assetLoader.shouldInterceptRequest(
                        Uri.parse("https://appassets.androidplatform.net/assets/index.html")
                    )
                } else null
            }

            override fun shouldOverrideUrlLoading(view: WebView?, url: String?): Boolean {
                url?.let { view?.loadUrl(it) }
                return true
            }
        }
    }

    private fun loadUrl() {
        webView.loadUrl("https://appassets.androidplatform.net/assets/index.html")
    }

    private fun setupSystemBars() {
        val isDarkTheme = resources.configuration.uiMode and
                Configuration.UI_MODE_NIGHT_MASK ==
                Configuration.UI_MODE_NIGHT_YES

        if (isDarkTheme) {
            window.statusBarColor = Color.BLACK
            window.navigationBarColor = Color.BLACK
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
                window.insetsController?.setSystemBarsAppearance(
                    0,
                    WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS or
                            WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS
                )
            } else if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
                @Suppress("DEPRECATION")
                window.decorView.systemUiVisibility =
                    window.decorView.systemUiVisibility and View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR.inv()
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    @Suppress("DEPRECATION")
                    window.decorView.systemUiVisibility =
                        window.decorView.systemUiVisibility and View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR.inv()
                }
            }
        } else {
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
        setupSystemBars()
        setupWebViewBackground()
        setupRootBackground()
    }

    private fun setupWebViewBackground() {
        val isDarkTheme = resources.configuration.uiMode and
                Configuration.UI_MODE_NIGHT_MASK ==
                Configuration.UI_MODE_NIGHT_YES
        webView.setBackgroundColor(if (isDarkTheme) Color.BLACK else Color.WHITE)
    }

    private fun setupRootBackground() {
        val isDarkTheme = resources.configuration.uiMode and
                Configuration.UI_MODE_NIGHT_MASK ==
                Configuration.UI_MODE_NIGHT_YES
        val rootView = findViewById<View>(android.R.id.content)
        rootView.setBackgroundColor(if (isDarkTheme) Color.BLACK else Color.WHITE)
    }
}
