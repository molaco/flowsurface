#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.8"
# dependencies = [
#     "elevenlabs",
#     "python-dotenv",
# ]
# ///

#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.8"
# dependencies = [
#     "elevenlabs",
#     "python-dotenv",
# ]
# ///

# import os
# import sys
# from pathlib import Path
# from dotenv import load_dotenv
#
#
# def main():
#     """
#     ElevenLabs Turbo v2.5 TTS Script
#
#     Uses ElevenLabs' Turbo v2.5 model for fast, high-quality text-to-speech.
#     Accepts optional text prompt as command-line argument.
#
#     Usage:
#     - ./eleven_turbo_tts.py                    # Uses default text
#     - ./eleven_turbo_tts.py "Your custom text" # Uses provided text
#
#     Features:
#     - Fast generation (optimized for real-time use)
#     - High-quality voice synthesis
#     - Stable production model
#     - Cost-effective for high-volume usage
#     """
#
#     # Load environment variables
#     load_dotenv()
#
#     # Get API key from environment
#     api_key = os.getenv("ELEVENLABS_API_KEY")
#     if not api_key:
#         print("❌ Error: ELEVENLABS_API_KEY not found in environment variables")
#         print("Please add your ElevenLabs API key to .env file:")
#         print("ELEVENLABS_API_KEY=your_api_key_here")
#         sys.exit(1)
#
#     try:
#         from elevenlabs.client import ElevenLabs
#         from elevenlabs import play
#
#         # Initialize client
#         elevenlabs = ElevenLabs(api_key=api_key)
#
#         print("🎙️  ElevenLabs Turbo v2.5 TTS")
#         print("=" * 40)
#
#         # Get text from command line argument or use default
#         if len(sys.argv) > 1:
#             text = " ".join(sys.argv[1:])  # Join all arguments as text
#         else:
#             text = "The first move is what sets everything in motion."
#
#         print(f"🎯 Text: {text}")
#         print("🔊 Generating and playing...")
#
#         try:
#             # Generate and play audio directly
#             audio = elevenlabs.text_to_speech.convert(
#                 text=text,
#                 voice_id="vGQNBgLaiM3EdZtxIiuY",  # Specified voice
#                 model_id="eleven_turbo_v2_5",
#                 output_format="mp3_44100_128",
#             )
#
#             play(audio)
#             print("✅ Playback complete!")
#
#         except Exception as e:
#             print(f"❌ Error: {e}")
#
#     except ImportError:
#         print("❌ Error: elevenlabs package not installed")
#         print("This script uses UV to auto-install dependencies.")
#         print("Make sure UV is installed: https://docs.astral.sh/uv/")
#         sys.exit(1)
#     except Exception as e:
#         print(f"❌ Unexpected error: {e}")
#         sys.exit(1)
#
#
# if __name__ == "__main__":
#     main()


import os
import sys
import subprocess
import shutil
import tempfile  # Add this at the top if not already imported
from pathlib import Path
from dotenv import load_dotenv


def main():
    """
    ElevenLabs Turbo v2.5 TTS Script

    Uses ElevenLabs' Turbo v2.5 model for fast, high-quality text-to-speech.
    Accepts optional text prompt as command-line argument.

    Usage:
    - ./eleven_turbo_tts.py                    # Uses default text
    - ./eleven_turbo_tts.py "Your custom text" # Uses provided text

    Features:
    - Fast generation (optimized for real-time use)
    - High-quality voice synthesis
    - Stable production model
    - Cost-effective for high-volume usage
    """

    # Load environment variables
    load_dotenv()

    # Get API key from environment
    api_key = os.getenv("ELEVENLABS_API_KEY")
    if not api_key:
        print("❌ Error: ELEVENLABS_API_KEY not found in environment variables")
        print("Please add your ElevenLabs API key to .env file:")
        print("ELEVENLABS_API_KEY=your_api_key_here")
        sys.exit(1)

    try:
        from elevenlabs.client import ElevenLabs
        from elevenlabs import play

        # Initialize client
        elevenlabs = ElevenLabs(api_key=api_key)

        print("🎙️  ElevenLabs Turbo v2.5 TTS")
        print("=" * 40)

        # Get text from command line argument or use default
        if len(sys.argv) > 1:
            text = " ".join(sys.argv[1:])  # Join all arguments as text
        else:
            text = "The first move is what sets everything in motion."

        print(f"🎯 Text: {text}")
        print("🔊 Generating and playing...")

        try:
            # Generate and play audio directly
            audio = elevenlabs.text_to_speech.convert(
                text=text,
                voice_id="vGQNBgLaiM3EdZtxIiuY",  # Specified voice
                model_id="eleven_flash_v2_5",
                output_format="mp3_44100_128",
            )

            filename = "out.mp3"
            win_dir = "/mnt/c/Users/Public"  # Or another shared folder on C: drive

            # Save audio in Linux path
            with open(filename, "wb") as f:
                for chunk in audio:
                    f.write(chunk)

            # Copy MP3 to Windows path
            target_path = os.path.join(win_dir, filename)
            shutil.copy(filename, target_path)

            # Convert to Windows path (e.g., C:\Users\Public\out.mp3)
            win_path = (
                subprocess.check_output(["wslpath", "-w", target_path]).decode().strip()
            )

            # Normalize to forward slashes for URI (file:///C:/Users/Public/out.mp3)
            uri_path = win_path.replace("\\", "/")

            # Define the PowerShell playback script content
            ps_content = f"""
            Add-Type -AssemblyName PresentationCore
            $mp = New-Object System.Windows.Media.MediaPlayer
            $mp.Open([uri]"file:///{uri_path}")
            do {{ Start-Sleep -Milliseconds 50 }} until ($mp.NaturalDuration.HasTimeSpan)
            $mp.Volume = 0.1
            $mp.Play()
            Start-Sleep -Milliseconds $mp.NaturalDuration.TimeSpan.TotalMilliseconds
            $mp.Close()
            """

            # Create temp .ps1 file in Windows dir
            ps1_filename = "play_audio.ps1"
            target_ps1_path = os.path.join(win_dir, ps1_filename)
            with open(target_ps1_path, "w") as f:
                f.write(ps_content)

            # Convert .ps1 to Windows path
            win_ps1_path = (
                subprocess.check_output(["wslpath", "-w", target_ps1_path])
                .decode()
                .strip()
            )

            # Run the .ps1 hidden directly (no Start-Process needed)
            os.system(
                f'powershell.exe -WindowStyle Hidden -NoProfile -ExecutionPolicy Bypass -File "{win_ps1_path}"'
            )

            # Clean up files
            os.remove(target_path)  # MP3
            os.remove(target_ps1_path)  # .ps1
            if os.path.exists(filename):  # Local MP3 if still there
                os.remove(filename)

            print("✅ Playback complete!")

        except Exception as e:
            print(f"❌ Error: {e}")

    except ImportError:
        print("❌ Error: elevenlabs package not installed")
        print("This script uses UV to auto-install dependencies.")
        print("Make sure UV is installed: https://docs.astral.sh/uv/")
        sys.exit(1)
    except Exception as e:
        print(f"❌ Unexpected error: {e}")
        sys.exit(1)


if __name__ == "__main__":
    main()
