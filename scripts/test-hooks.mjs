// Lets `node --test` run the TypeScript sources directly (Node strips the types itself):
// resolves extensionless relative imports to .ts and the @shared/* alias to src/shared.
import { registerHooks } from 'node:module'
import { existsSync } from 'node:fs'
import { fileURLToPath, pathToFileURL } from 'node:url'

const shared = new URL('../src/shared/', import.meta.url)

registerHooks({
  resolve(specifier, context, next) {
    if (specifier.startsWith('@shared/')) specifier = new URL(specifier.slice(8) + '.ts', shared).href
    else if ((specifier.startsWith('./') || specifier.startsWith('../')) && !/\.[cm]?[jt]s$/.test(specifier) && context.parentURL) {
      const candidate = new URL(specifier + '.ts', context.parentURL)
      if (existsSync(fileURLToPath(candidate))) specifier = pathToFileURL(fileURLToPath(candidate)).href
    }
    return next(specifier, context)
  }
})
