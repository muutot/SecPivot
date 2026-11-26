/** KeePass AceUI.RepeatPasswordOnlyWhenHidden, evaluated at submission time. */
export function passwordRepeatMatches(
  password: string,
  confirm: string,
  showPassword: boolean,
  onlyWhenHidden: boolean,
): boolean {
  return (onlyWhenHidden && showPassword) || password === confirm;
}
