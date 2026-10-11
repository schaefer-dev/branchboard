import { isMac } from "../platform/os";
import { CommandRegistry } from "./registry";

export const commands = new CommandRegistry(isMac);
export type { Command, Shortcut } from "./registry";
