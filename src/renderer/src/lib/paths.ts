let home = ''

/** Called once at startup with the user's home folder. */
export const setHome = (h: string): void => {
  home = h.replace(/\/$/, '')
}

/** /home/me/Documents → ~/Documents */
export const tilde = (p: string): string => (home && (p === home || p.startsWith(home + '/')) ? '~' + p.slice(home.length) : p)
