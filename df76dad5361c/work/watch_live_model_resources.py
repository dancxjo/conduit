import runpy,time,pathlib
while any(pathlib.Path('/proc') .joinpath(str(p)).exists() for p in [2898759,3407194,3460414]):
 try: runpy.run_path('work/capture_live_model_resources.py')
 except FileNotFoundError: pass
 time.sleep(5)
