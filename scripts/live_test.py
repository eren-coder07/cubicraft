"""Test the LIVE GitHub Pages deployment of CubiCraft."""
import asyncio
from playwright.async_api import async_playwright

async def main():
    async with async_playwright() as p:
        browser = await p.chromium.launch(args=[
            "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader",
            "--enable-webgl", "--ignore-gpu-blocklist"])
        page = await browser.new_page(viewport={"width": 1280, "height": 800})
        logs = []
        page.on("console", lambda m: logs.append(f"[{m.type}] {m.text[:120]}"))
        page.on("pageerror", lambda e: logs.append(f"[pageerror] {str(e)[:160]}"))
        await page.goto("https://eren-coder07.github.io/cubicraft/", wait_until="load")
        await page.wait_for_timeout(6000)
        # interact: track mouse right, then play a motion
        await page.mouse.move(900, 350)
        await page.wait_for_timeout(1000)
        await page.screenshot(path="/tmp/shots/live-track.png")
        await page.mouse.click(108, 771)  # Wave
        await page.wait_for_timeout(900)
        await page.screenshot(path="/tmp/shots/live-wave.png")
        print("LIVE CONSOLE:")
        for l in logs[-10:]:
            print(" ", l)
        await browser.close()

asyncio.run(main())
