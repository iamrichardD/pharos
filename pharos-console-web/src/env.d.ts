/// <reference path="../.astro/types.d.ts" />
/// <reference types="astro/client" />

declare namespace App {
    interface Locals {
        session?: import('./features/auth/jwt-logic').UserSession;
    }
}
