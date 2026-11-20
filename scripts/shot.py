"""Headless test driver for CubiCraft web build."""
import asyncio, sys, json
from playwright.async_api import async_playwright

async def main():
    shots = sys.argv[1:] or ["default"]
    async with async_playwright() as p:
        browser = await p.chromium.launch(args=[
            "--use-gl=angle", "--use-angle=swiftshader", "--enable-unsafe-swiftshader",
            "--enable-webgl", "--ignore-gpu-blocklist"])
        page = await browser.new_page(viewport={"width": 1280, "height": 800})
        logs = []
        page.on("console", lambda m: logs.append(f"[{m.type}] {m.text}"))
        page.on("pageerror", lambda e: logs.append(f"[pageerror] {e}"))
        await page.goto("http://127.0.0.1:8137/index.html")
        await page.wait_for_timeout(4500)
        for name in shots:
            if name == "mouse":
                await page.mouse.move(700, 380)
                await page.wait_for_timeout(1200)
            elif name == "grid":
                await page.mouse.click(47, 71)   # Grid toggle
                await page.wait_for_timeout(700)
            elif name == "mesh":
                await page.mouse.click(123, 71)  # Mesh toggle
                await page.wait_for_timeout(700)
            elif name == "panel":
                await page.mouse.click(1000, 260)  # slider area
                await page.wait_for_timeout(500)
            elif name == "smile":
                await page.mouse.click(370, 771)   # Smile emote

                await page.mouse.move(640, 700)
                await page.wait_for_timeout(900)
            elif name == "wave":
                await page.mouse.click(108, 771)   # Wave motion
                await page.wait_for_timeout(900)
            elif name == "hop":
                await page.mouse.click(190, 771)   # Hop motion
                await page.wait_for_timeout(500)
            elif name == "select":
                await page.mouse.move(770, 320)    # face
                await page.mouse.down(); await page.mouse.up()
                await page.wait_for_timeout(600)
            elif name == "menu":
                await page.mouse.click(154, 38)    # Modeling menu
                await page.wait_for_timeout(500)
            elif name == "gridedit":
                await page.mouse.click(151, 68)    # Edit Level 3
                await page.mouse.click(285, 68)    # Grids toggle
                await page.wait_for_timeout(400)
                # select bangs warp via deformer tree list (row: warp 6x4 under head)
                await page.mouse.click(116, 652)
                await page.wait_for_timeout(600)
                await page.mouse.move(640, 190)    # grab a top grid point
                await page.mouse.down()
                await page.mouse.move(660, 160, steps=8)
                await page.mouse.up()
                await page.wait_for_timeout(500)
            elif name == "params":
                await page.mouse.click(439, 651)   # Parameter tab
                await page.wait_for_timeout(400)
                await page.mouse.move(1150, 230)   # hover slider area
                await page.mouse.down()
                await page.mouse.move(1100, 230, steps=6)
                await page.mouse.up()
                await page.wait_for_timeout(400)
            await page.screenshot(path=f"/tmp/shots/{name}.png")
            print(f"shot: {name}")
        print("CONSOLE LOGS:")
        for l in logs[-30:]:
            print(" ", l)
        await browser.close()

asyncio.run(main())
