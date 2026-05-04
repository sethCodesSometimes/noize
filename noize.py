#!/usr/bin/env python3
import mpv
import sys
import os
import json

class AudioMixer:
    def __init__(self, config_path="config.json"):
        self.sources = []
        self.current_index = 0
        self.max_volume = 10
        self.volume_step = 1
        self.load_config(config_path)
        
    def load_config(self, config_path):
        with open(config_path) as f:
            config = json.load(f)
        self.max_volume = config.get("max_volume", 10)
        self.volume_step = config.get("volume_step", 1)
        for src in config.get("sources", []):
            urls = src["url"] if isinstance(src["url"], list) else [src["url"]]
            self.add_source(urls, src.get("name", ""))
        
    def add_source(self, urls, name=None):
        if isinstance(urls, str):
            urls = [urls]
        is_local = all(os.path.isfile(u) for u in urls)
        player = mpv.MPV(ytdl=not is_local, video=False)
        
        source_data = {
            'player': player,
            'name': name or (urls[0][:40] if urls else "Unknown"),
            'volume': 0,
            'urls': urls,
            'url_index': 0,
            'muted': False,
            'previous_volume': 0
        }
        
        self.sources.append(source_data)
        player.play(urls[0])
        player.volume = 0
        
        def try_next_url():
            if source_data['url_index'] + 1 < len(urls):
                source_data['url_index'] += 1
                player.play(urls[source_data['url_index']])
                return True
            return False
        
        @player.event_callback('end-file')
        def on_end_file(event):
            if hasattr(event, 'reason') and event.reason in [2, 3, 4]:
                try_next_url()
        
    def adjust_volume(self, delta):
        if not self.sources:
            return
        source = self.sources[self.current_index]
        new_vol = max(0, min(self.max_volume, source['volume'] + delta))
        source['volume'] = new_vol
        perceived = self.perceived_volume(new_vol)
        source['player'].volume = perceived
        
    def perceived_volume(self, step):
        return 100 * (step / self.max_volume)
        
    def toggle_mute(self):
        if not self.sources:
            return
        source = self.sources[self.current_index]
        if source.get('muted'):
            source['volume'] = source.get('previous_volume', 0)
            source['player'].volume = self.perceived_volume(source['volume'])
            source['muted'] = False
        else:
            source['previous_volume'] = source['volume']
            source['volume'] = 0
            source['player'].volume = 0
            source['muted'] = True
        
    def scroll_up(self):
        if self.sources:
            self.current_index = (self.current_index - 1) % len(self.sources)
            
    def scroll_down(self):
        if self.sources:
            self.current_index = (self.current_index + 1) % len(self.sources)
            
    def display(self):
        os.system('clear')
        print("=== noize ===")
        for i, src in enumerate(self.sources):
            marker = ">" if i == self.current_index else " "
            vol = src['previous_volume'] if src.get('muted') else src['volume']
            bar = "#" * vol + "-" * (self.max_volume - vol)
            mute_indicator = " [MUTED]" if src.get('muted') else ""
            url_info = f" [{src['url_index']+1}/{len(src['urls'])}]" if len(src['urls']) > 1 else ""
            print(f"{marker} {src['name'][:30]:30} [{bar}] {vol}/{self.max_volume}{mute_indicator}{url_info}")
        print("\nControls: j/k=scroll, h/l=volume down/up, m=mute/unmute, q=quit")
        
    def run(self):
        while True:
            self.display()
            try:
                key = sys.stdin.read(1).lower()
                if key == 'q':
                    break
                elif key == 'j':
                    self.scroll_down()
                elif key == 'k':
                    self.scroll_up()
                elif key == 'h':
                    self.adjust_volume(-self.volume_step)
                elif key == 'l':
                    self.adjust_volume(self.volume_step)
                elif key == 'm':
                    self.toggle_mute()
            except KeyboardInterrupt:
                break
                
        for src in self.sources:
            src['player'].terminate()

if __name__ == "__main__":
    import tty
    import termios
    import argparse
    
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="config.json", help="Path to config file")
    args = parser.parse_args()
    
    mixer = AudioMixer(args.config)
    
    old_settings = termios.tcgetattr(sys.stdin)
    try:
        tty.setcbreak(sys.stdin.fileno())
        mixer.run()
    finally:
        termios.tcsetattr(sys.stdin, termios.TCSADRAIN, old_settings)
