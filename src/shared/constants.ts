// Rename the app in one place. Keep APP_ID in sync with `appId` in electron-builder.yml.
export const APP_NAME = 'Lumen'
export const APP_ID = 'io.github.lumenlauncher.Lumen'
export const DBUS_NAME = APP_ID
export const DBUS_PATH = '/' + APP_ID.replace(/\./g, '/')
export const DBUS_IFACE = APP_ID

export const DEFAULT_SHORTCUT = '<Alt>space'

export const PANEL_WIDTH = 760
/** Where the panel's top edge sits, as a fraction of the screen height (Spotlight is ~22%). */
export const PANEL_TOP = 0.2

export const SEARCH_ENGINES = {
  google: { name: 'Google', url: 'https://www.google.com/search?q=%s' },
  duckduckgo: { name: 'DuckDuckGo', url: 'https://duckduckgo.com/?q=%s' },
  brave: { name: 'Brave Search', url: 'https://search.brave.com/search?q=%s' },
  bing: { name: 'Bing', url: 'https://www.bing.com/search?q=%s' },
  startpage: { name: 'Startpage', url: 'https://www.startpage.com/do/search?q=%s' }
} as const

/** Type `yt cats` to search YouTube for cats, etc. */
export const WEB_KEYWORDS: Record<string, { name: string; url: string }> = {
  g: { name: 'Google', url: 'https://www.google.com/search?q=%s' },
  ddg: { name: 'DuckDuckGo', url: 'https://duckduckgo.com/?q=%s' },
  yt: { name: 'YouTube', url: 'https://www.youtube.com/results?search_query=%s' },
  gh: { name: 'GitHub', url: 'https://github.com/search?q=%s&type=repositories' },
  wiki: { name: 'Wikipedia', url: 'https://en.wikipedia.org/wiki/Special:Search?search=%s' },
  maps: { name: 'Google Maps', url: 'https://www.google.com/maps/search/%s' },
  npm: { name: 'npm', url: 'https://www.npmjs.com/search?q=%s' },
  so: { name: 'Stack Overflow', url: 'https://stackoverflow.com/search?q=%s' },
  tr: { name: 'Google Translate', url: 'https://translate.google.com/?sl=auto&tl=en&text=%s' },
  img: { name: 'Google Images', url: 'https://www.google.com/search?tbm=isch&q=%s' },
  reddit: { name: 'Reddit', url: 'https://www.reddit.com/search/?q=%s' },
  mdn: { name: 'MDN', url: 'https://developer.mozilla.org/en-US/search?q=%s' }
}
