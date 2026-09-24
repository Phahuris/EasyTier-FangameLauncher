# FGL_Net v1 — FR

module FGL
  DIR = "FGL_peers"
  TICK = 0.05
  STALE_KILL = 1800.0
  FW = 80
  FH = 80
  @my_id = nil
  @last = 0.0
  @last_log = 0.0
  @last_scene_log = 0.0
  @players = {}
  @hooks_done = false
  @local_fishing = false
  @last_action_written = nil

  def self.log(msg); end

  def self.ensure_id
    return if @my_id
    @my_id = "#{Time.now.to_i}_#{rand(999999)}"
    begin
      Dir.mkdir(DIR) unless File.directory?(DIR)
    rescue
    end
  end

  def self.map_id
    begin
      return $game_map.map_id
    rescue
      return 0
    end
  end

  def self.factory
    begin
      return $MapFactory if defined?($MapFactory) && $MapFactory
    rescue
    end
    begin
      return $map_factory if defined?($map_factory) && $map_factory
    rescue
    end
    nil
  end

  def self.clean_name(path)
    return "" if path.nil? || path.to_s.empty?
    s = path.to_s.gsub("\\", "/")
    s = s.sub(%r{^Graphics/Characters/}i, "")
    s = s.sub(%r{^/+}, "")
    s = s.sub(/\.(png|jpg|bmp)$/i, "")
    s.gsub("|", "")
  end

  def self.safe_text(s)
    s.to_s.gsub("|", "").gsub("\n", " ").strip[0, 32]
  end

  def self.player_display_name
    begin
      return safe_text($player.name) if defined?($player) && $player && $player.name
    rescue
    end
    begin
      return safe_text($Trainer.name) if defined?($Trainer) && $Trainer && $Trainer.name
    rescue
    end
    "Joueur"
  end

  def self.ui_locks_peers?
    begin
      return true if $game_map && $game_map.scrolling?
    rescue
    end
    begin
      if $game_map && $game_player
        sx, sy = tile_to_screen($game_player.x, $game_player.y)
        screen_cx = Graphics.width / 2
        screen_cy = Graphics.height / 2
        if (sx - screen_cx).abs > 32 || (sy - screen_cy).abs > 32
          return true
        end
      end
    rescue
    end
    begin
      return true if $scene && !$scene.is_a?(Scene_Map)
    rescue
    end
    begin
      if defined?($game_temp) && $game_temp
        return true if $game_temp.respond_to?(:in_menu) && $game_temp.in_menu
        return true if $game_temp.respond_to?(:menu_calling) && $game_temp.menu_calling
        return true if $game_temp.respond_to?(:common_event_id) && $game_temp.common_event_id.to_i > 0
      end
    rescue
    end
    begin
      return true if defined?($PokemonTemp) && $PokemonTemp && $PokemonTemp.trainer_preview
    rescue
    end
    begin
      if defined?(SWITCH_SELECTING_CLOTHES) && $game_switches[SWITCH_SELECTING_CLOTHES]
        return true
      end
    rescue
    end
    false
  end

  def self.detect_move_state
    begin
      return 3 if @local_fishing
      return 3 if defined?($PokemonGlobal) && $PokemonGlobal && $PokemonGlobal.fishing
    rescue
    end
    begin
      return 1 if defined?($PokemonGlobal) && $PokemonGlobal && $PokemonGlobal.surfing
      return 4 if defined?($PokemonGlobal) && $PokemonGlobal && $PokemonGlobal.diving
      return 2 if defined?($PokemonGlobal) && $PokemonGlobal && $PokemonGlobal.bicycle
    rescue
    end
    begin
      cn = $game_player.character_name.to_s.downcase
      return 1 if cn.include?("surf")
      return 4 if cn.include?("dive")
      return 2 if cn.include?("bike")
      return 3 if cn.include?("fish")
    rescue
    end
    0
  end

  def self.state_to_action(state, cname)
    cn = cname.to_s.downcase
    return "surf" if state.to_i == 1 || cn.include?("surf")
    return "dive" if state.to_i == 4 || cn.include?("dive")
    return "bike" if state.to_i == 2 || cn.include?("bike")
    return "fish" if state.to_i == 3 || cn.include?("fish")
    return "run" if cn.include?("run")
    "walk"
  end

  def self.detect_action
    sc = ""
    begin
      sc = $scene.class.name.to_s if $scene
    rescue
    end
    begin
      return "Peche" if @local_fishing
      return "Peche" if defined?($PokemonGlobal) && $PokemonGlobal && $PokemonGlobal.fishing
    rescue
    end
    begin
      if defined?(FGLBattle)
        return "Combat" if (FGLBattle.busy? rescue false) || (FGLBattle.active? rescue false)
      end
    rescue
    end
    begin
      return "Combat" if sc.include?("PokeBattle") || sc.include?("Battle_Scene") ||
                         (sc.include?("Battle") && !sc.include?("Map") && !sc.include?("Bag"))
      return "Combat" if defined?(pbInBattle?) && pbInBattle?
    rescue
    end
    begin
      if defined?(FGLTrade)
        return "Echange" if (FGLTrade.busy? rescue false) || (FGLTrade.active? rescue false)
      end
    rescue
    end
    begin
      return "Echange" if sc.include?("Trade") || sc.include?("PokemonTrade")
    rescue
    end
    begin
      return "Menu" if defined?($game_temp) && $game_temp && $game_temp.respond_to?(:in_menu) && $game_temp.in_menu
      return "Menu" if sc.include?("PauseMenu")
    rescue
    end
    ""
  end

  def self.read_trainer_outfit
    clothes = hair = hat = hat2 = ""
    cc = hc = htc = h2c = skin = 0
    surfmon = ""
    bike_col = 0
    begin
      if defined?($Trainer) && $Trainer
        t = $Trainer
        clothes = t.clothes.to_s if t.respond_to?(:clothes)
        hair    = t.hair.to_s if t.respond_to?(:hair)
        hat     = t.hat.to_s if t.respond_to?(:hat)
        hat2    = t.hat2.to_s if t.respond_to?(:hat2)
        cc  = t.clothes_color.to_i if t.respond_to?(:clothes_color)
        hc  = t.hair_color.to_i if t.respond_to?(:hair_color)
        htc = t.hat_color.to_i if t.respond_to?(:hat_color)
        h2c = t.hat2_color.to_i if t.respond_to?(:hat2_color)
        skin = t.skin_tone.to_i if t.respond_to?(:skin_tone)
        bike_col = t.bike_color.to_i if t.respond_to?(:bike_color)
        if t.respond_to?(:surfing_pokemon) && t.surfing_pokemon
          begin
            sp = t.surfing_pokemon
            surfmon = sp.respond_to?(:species) ? sp.species.to_s : sp.to_s
          rescue
            surfmon = t.surfing_pokemon.to_s
          end
        end
      end
    rescue
    end
    [safe_text(clothes), safe_text(hair), safe_text(hat), safe_text(hat2),
     cc, hc, htc, h2c, skin, safe_text(surfmon), bike_col]
  end

  def self.target_map_for(remote_mid)
    return nil if !remote_mid || remote_mid == 0
    begin
      return $game_map if $game_map && remote_mid == $game_map.map_id
    rescue
    end
    mf = factory
    return nil unless mf
    begin
      mf.setMapsInRange if mf.respond_to?(:setMapsInRange)
    rescue
    end
    begin
      return mf.getMap(remote_mid) if mf.respond_to?(:hasMap?) && mf.hasMap?(remote_mid)
    rescue
    end
    begin
      if mf.respond_to?(:areConnected?) && $game_map && mf.areConnected?($game_map.map_id, remote_mid)
        return mf.getMap(remote_mid)
      end
    rescue
    end
    nil
  end

  def self.vp
    begin
      return Spriteset_Map.viewport if defined?(Spriteset_Map) && Spriteset_Map.respond_to?(:viewport)
    rescue
    end
    return nil unless $scene
    begin
      if $scene.respond_to?(:spriteset) && $scene.spriteset
        s = $scene.spriteset
        return s.viewport1 if s.respond_to?(:viewport1)
        return s.instance_variable_get(:@viewport1) rescue nil
      end
    rescue
    end
    nil
  end

  def self.safe_dispose(obj)
    return unless obj
    begin
      obj.dispose unless (obj.disposed? rescue false)
    rescue
    end
  end

  def self.destroy_player_visuals(rec)
    return unless rec
    [:sprite, :hair_spr, :hat_spr, :hat2_spr, :bike_spr, :surf_sprite,
     :label_name, :label_action].each { |k| safe_dispose(rec[k]); rec[k] = nil }
    [:owned_bmp, :hair_bmp, :hat_bmp, :hat2_bmp, :bike_bmp,
     :label_name_bmp, :label_action_bmp].each do |k|
      begin
        rec[k].dispose if rec[k]
      rescue
      end
      rec[k] = nil
    end
    begin
      rec[:surf_anim].dispose if rec[:surf_anim] && rec[:surf_anim].respond_to?(:dispose)
    rescue
    end
    rec[:surf_anim] = nil
    rec[:label_key] = nil
    rec[:bound_map_id] = nil
    rec[:last_outfit_key] = nil
    rec[:frozen_sx] = nil
    rec[:frozen_sy] = nil
  end

  def self.tile_to_screen(tx, ty, map = nil)
    map = $game_map if map.nil?
    begin
      if defined?(Game_Map::REAL_RES_X) && defined?(Game_Map::X_SUBPIXELS)
        sx = ((tx * Game_Map::REAL_RES_X - map.display_x) / Game_Map::X_SUBPIXELS).ceil
        sy = ((ty * Game_Map::REAL_RES_Y - map.display_y) / Game_Map::Y_SUBPIXELS).ceil
        tw = defined?(Game_Map::TILE_WIDTH) ? Game_Map::TILE_WIDTH : 32
        th = defined?(Game_Map::TILE_HEIGHT) ? Game_Map::TILE_HEIGHT : 32
        return [sx + tw / 2, sy + th]
      end
    rescue
    end
    begin
      return [(tx * 32) - (map.display_x / 4) + 16,
              (ty * 32) - (map.display_y / 4) + 32]
    rescue
      return [0, 0]
    end
  end

  def self.calc_z(sy, remote_y = nil)
    z = sy + 32
    begin
      if remote_y && $game_player
        if remote_y > $game_player.y
          z += 20
        elsif remote_y < $game_player.y
          z -= 20
        end
      end
    rescue
    end
    z
  end

  def self.offset_for(action, dir, frame)
    f = frame.to_i; f = 0 if f < 0; f = 3 if f > 3
    d = dir.to_i; d = 2 if d <= 0
    case action.to_s
    when "surf"
      table = {2 => [[0,-6],[0,-4],[0,-6],[0,-4]], 4 => [[-2,-10],[-2,-8],[-2,-10],[-2,-8]],
               6 => [[2,-10],[2,-8],[2,-10],[2,-8]], 8 => [[0,-10],[0,-8],[0,-10],[0,-8]]}
      f = 0
    when "dive"
      table = {2 => [[0,-6],[0,-4],[0,-6],[0,-4]], 4 => [[6,-8],[6,-6],[6,-8],[6,-6]],
               6 => [[-6,-8],[-6,-6],[-6,-8],[-6,-6]], 8 => [[0,-2],[0,0],[0,-2],[0,0]]}
    when "bike"
      table = {2 => [[0,-2],[2,0],[0,-2],[-2,0]], 4 => [[-4,-4],[-2,-2],[-4,-4],[-6,-2]],
               6 => [[4,-4],[2,-2],[4,-4],[6,-2]], 8 => [[0,-2],[-2,0],[0,-2],[2,0]]}
    when "fish"
      table = {2 => [[0,-6],[0,-2],[0,-8],[2,-6]], 4 => [[0,-8],[-6,-6],[0,-8],[2,-8]],
               6 => [[0,-8],[6,-6],[0,-8],[-2,-8]], 8 => [[0,-6],[0,-6],[0,-6],[2,-4]]}
    when "run"
      table = {2 => [[0,2],[0,6],[0,2],[0,6]], 4 => [[-2,-2],[-2,-2],[-2,-2],[-2,-2]],
               6 => [[2,-2],[2,-2],[2,-2],[2,-2]], 8 => [[0,-2],[0,-2],[0,-2],[0,-2]]}
    else
      return [0, 0]
    end
    arr = table[d] || table[2]
    arr ? arr[f] : [0, 0]
  end

  def self.draw_rounded_rect(bmp, x, y, w, h, col)
    return if w < 6 || h < 6
    bmp.fill_rect(x + 2, y, w - 4, h, col)
    bmp.fill_rect(x, y + 2, w, h - 4, col)
    bmp.fill_rect(x + 1, y + 1, w - 2, h - 2, col)
  end

  def self.make_bubble(text, is_action)
    text = safe_text(text)
    text = "?" if text.empty?
    w = 96; h = 18
    bmp = Bitmap.new(w, h)
    if is_action
      bg = Color.new(20, 20, 20, 170)
      fg = Color.new(255, 220, 80)
    else
      bg = Color.new(0, 0, 0, 150)
      fg = Color.new(255, 255, 255)
    end
    draw_rounded_rect(bmp, 2, 1, w - 4, h - 2, bg)
    bmp.font.name = "Arial"
    bmp.font.size = 14
    bmp.font.color = fg
    bmp.draw_text(0, 1, w, 16, text, 1)
    bmp
  end

  def self.apply_tint(spr)
    return unless spr
    begin
      pbDayNightTint(spr) if defined?(pbDayNightTint)
    rescue
    end
  end

  def self.write_me
    return unless $game_player && $game_map
    ensure_id
    p = $game_player
    cname = clean_name(p.character_name) rescue "walk"
    cname = "walk" if cname.empty?
    px = p.x.to_i rescue 0
    py = p.y.to_i rescue 0
    pdir = p.direction.to_i rescue 2
    pspeed = p.move_speed.to_i rescue 3
    ppat = p.pattern.to_i rescue 0
    begin
      ppat = p.pattern_surf.to_i if detect_move_state == 1 && p.respond_to?(:pattern_surf)
    rescue
    end
    pname = player_display_name
    action = safe_text(detect_action)
    if action != @last_action_written
      @last_action_written = action
    end
    state = detect_move_state
    clothes, hair, hat, hat2, cc, hc, htc, h2c, skin, surfmon, bike_col = read_trainer_outfit
    line = [
      @my_id, "P", map_id, px, py, pdir, cname, pspeed, ppat, 0,
      0, 0, 0, 0, 0, action, pname,
      clothes, hair, hat, hat2, cc, hc, htc, h2c, skin, state, surfmon, bike_col, Time.now.to_i
    ].join("|")
    begin
      File.open(File.join(DIR, "#{@my_id}.txt"), "wb") { |f| f.write(line) }
    rescue
    end
  end

  def self.read_others
    ensure_id
    return unless File.directory?(DIR)
    seen = {}
    nowt = Time.now
    Dir.foreach(DIR) do |fn|
      next if fn == "." || fn == ".." || fn[-4, 4] != ".txt"
      id = fn[0, fn.length - 4]
      next if id == @my_id
      next if id =~ /^(chal|chal_ans|chalresp|trd_|bat_|party_)/i
      path = File.join(DIR, fn)
      begin
        next if (nowt - File.mtime(path)) > STALE_KILL
      rescue
        next
      end
      raw = nil
      begin
        File.open(path, "rb") { |f| raw = f.read }
      rescue
        next
      end
      next if !raw || raw.empty?
      a = raw.strip.split("|")
      next if a.size < 7
      next if a[1].to_s != "P" && a[1].to_s != ""
      seen[id] = true
      data = {
        :map => a[2].to_i, :x => a[3].to_i, :y => a[4].to_i, :dir => a[5].to_i,
        :cname => clean_name(a[6]), :speed => a[7].to_i, :pattern => a[8].to_i,
        :action => (a.size > 15 ? safe_text(a[15]) : ""),
        :pname => (a.size > 16 ? safe_text(a[16]) : "Joueur"),
        :clothes => (a.size > 17 ? safe_text(a[17]) : ""),
        :hair => (a.size > 18 ? safe_text(a[18]) : ""),
        :hat => (a.size > 19 ? safe_text(a[19]) : ""),
        :hat2 => (a.size > 20 ? safe_text(a[20]) : ""),
        :cc => (a.size > 21 ? a[21].to_i : 0),
        :hc => (a.size > 22 ? a[22].to_i : 0),
        :htc => (a.size > 23 ? a[23].to_i : 0),
        :h2c => (a.size > 24 ? a[24].to_i : 0),
        :skin => (a.size > 25 ? a[25].to_i : 0),
        :state => (a.size > 26 ? a[26].to_i : 0),
        :surfmon => (a.size > 27 ? safe_text(a[27]) : ""),
        :bike_col => (a.size > 28 ? a[28].to_i : 0)
      }
      data[:pname] = "Joueur" if data[:pname].empty?
      if !@players[id]
        @players[id] = data.merge(
          :sprite => nil, :hair_spr => nil, :hat_spr => nil, :hat2_spr => nil,
          :bike_spr => nil, :surf_sprite => nil,
          :label_name => nil, :label_action => nil, :label_key => nil,
          :bound_map_id => nil, :miss => 0, :last_outfit_key => nil,
          :frozen_sx => nil, :frozen_sy => nil
        )
      else
        @players[id].merge!(data)
      end
    end
    @players.keys.each do |id|
      if !seen[id]
        @players[id][:miss] = (@players[id][:miss] || 0) + 1
        kill(id) if @players[id][:miss] > 600
      else
        @players[id][:miss] = 0
      end
    end
  end

  def self.build_body_bitmap(rec)
    action = state_to_action(rec[:state], rec[:cname])
    begin
      base_path = nil
      if defined?(getBaseOverworldSpriteFilename)
        base_path = getBaseOverworldSpriteFilename(action, rec[:skin].to_i) rescue nil
      end
      if !base_path || (defined?(pbResolveBitmap) && !pbResolveBitmap(base_path))
        base_path = Settings::PLAYER_GRAPHICS_FOLDER + action if defined?(Settings::PLAYER_GRAPHICS_FOLDER)
      end
      if !base_path || (defined?(pbResolveBitmap) && !pbResolveBitmap(base_path))
        base_path = "Graphics/Characters/#{action}"
      end
      base = AnimatedBitmap.new(base_path)
      out = base.bitmap.clone
      if defined?(getOverworldOutfitFilename)
        op = getOverworldOutfitFilename(rec[:clothes], action) rescue nil
        if (!op || !pbResolveBitmap(op)) && defined?(Settings::PLAYER_TEMP_OUTFIT_FALLBACK)
          op = getOverworldOutfitFilename(Settings::PLAYER_TEMP_OUTFIT_FALLBACK) rescue op
        end
        if op && pbResolveBitmap(op)
          ob = AnimatedBitmap.new(op, rec[:cc].to_i)
          out.blt(0, 0, ob.bitmap, ob.bitmap.rect)
        end
      end
      return out
    rescue
    end
    begin
      cn = rec[:cname].to_s
      cn = "walk" if cn.empty?
      return AnimatedBitmap.new("Graphics/Characters/#{cn}").bitmap.clone
    rescue
      return nil
    end
  end

  def self.load_layer_bitmap(path, hue)
    return nil if path.nil? || path.to_s == "" || path.to_s == "0"
    begin
      return nil if defined?(pbResolveBitmap) && !pbResolveBitmap(path)
      return AnimatedBitmap.new(path, hue.to_i).bitmap.clone
    rescue
      return nil
    end
  end

  def self.build_surfmon_anim(rec)
    begin
      species = nil
      if rec[:surfmon] && rec[:surfmon] != ""
        begin
          species = GameData::Species.get(rec[:surfmon].to_sym) if defined?(GameData::Species)
        rescue
        end
      end
      basePath = ""
      begin
        if defined?(Settings::PLAYER_GRAPHICS_FOLDER)
          basePath = Settings::PLAYER_GRAPHICS_FOLDER.to_s
          basePath += Settings::PLAYER_SURFBASE_FOLDER.to_s if Settings.const_defined?(:PLAYER_SURFBASE_FOLDER)
        end
      rescue
      end
      is_dive = (rec[:state].to_i == 4)
      act = is_dive ? "divemon" : "surfmon"
      candidates = []
      if species && species.respond_to?(:shape)
        candidates << "#{basePath}#{act}_#{species.shape.to_s}"
      end
      candidates << "#{basePath}#{act}_Head"
      candidates << "#{basePath}surfmon_board"
      candidates.each do |p|
        next if p.nil? || p == ""
        begin
          ok = defined?(pbResolveBitmap) ? pbResolveBitmap(p) : true
          return AnimatedBitmap.new(p) if ok
        rescue
        end
      end
    rescue
    end
    nil
  end

  def self.make_layer_sprite(v, bmp, z)
    return nil unless bmp
    s = ::Sprite.new(v)
    s.bitmap = bmp
    s.visible = true
    s.opacity = 255
    s.z = z
    apply_tint(s)
    s
  end

  def self.ensure_all(id)
    return if id.to_s == (@my_id.to_s rescue "")
    rec = @players[id]
    return unless rec
    begin
      return unless $game_map && $scene.is_a?(Scene_Map)
    rescue
      return
    end
    remote_mid = rec[:map].to_i
    tmap = target_map_for(remote_mid)
    if tmap.nil?
      destroy_player_visuals(rec) if rec[:sprite]
      return
    end
    rec[:_tmap] = tmap
    v = vp
    return if v.nil?

    action = state_to_action(rec[:state], rec[:cname])
    outfit_key = [rec[:cname], rec[:clothes], rec[:hair], rec[:hat], rec[:hat2],
                  rec[:cc], rec[:hc], rec[:htc], rec[:h2c], rec[:skin],
                  rec[:state], rec[:surfmon], rec[:bike_col]].join("|")
    need = !rec[:sprite]
    if rec[:sprite]
      begin
        need = true if rec[:sprite].disposed?
      rescue
        need = true
      end
    end
    if rec[:bound_map_id] != remote_mid || (rec[:last_outfit_key] && rec[:last_outfit_key] != outfit_key)
      destroy_player_visuals(rec)
      need = true
    end

    if need
      destroy_player_visuals(rec)
      body = build_body_bitmap(rec)
      return if body.nil?

      s = make_layer_sprite(v, body, 100)
      begin
        s.ox = FW / 2
        s.oy = FH
        dir = rec[:dir].to_i; dir = 2 if dir <= 0
        pat = rec[:pattern].to_i
        s.src_rect.set(pat * FW, ((dir - 2) / 2) * FH, FW, FH)
      rescue
      end
      rec[:sprite] = s
      rec[:owned_bmp] = body

      if defined?(getOverworldHairFilename) && rec[:hair].to_s != "" && rec[:hair].to_s != "0"
        hp = getOverworldHairFilename(rec[:hair]) rescue nil
        hb = load_layer_bitmap(hp, rec[:hc])
        if hb
          hs = make_layer_sprite(v, hb, 101)
          begin
            hs.ox = FW / 2; hs.oy = FH
          rescue
          end
          rec[:hair_spr] = hs
          rec[:hair_bmp] = hb
        end
      end

      [[:hat2, :h2c, :hat2_spr, :hat2_bmp, 102],
       [:hat,  :htc, :hat_spr,  :hat_bmp,  103]].each do |idk, colk, sprk, bmpk, zz|
        hid = rec[idk].to_s
        next if hid == "" || hid == "0"
        next unless defined?(getOverworldHatFilename)
        hpath = getOverworldHatFilename(hid) rescue nil
        hbm = load_layer_bitmap(hpath, rec[colk])
        if hbm
          hs = make_layer_sprite(v, hbm, zz)
          begin
            hs.ox = FW / 2; hs.oy = FH
          rescue
          end
          rec[sprk] = hs
          rec[bmpk] = hbm
        end
      end

      if action == "bike" && defined?(getOverworldBicycleFilename)
        begin
          bp = getOverworldBicycleFilename rescue nil
          bb = load_layer_bitmap(bp, rec[:bike_col])
          if bb
            bs = make_layer_sprite(v, bb, 99)
            begin
              bs.ox = FW / 2; bs.oy = FH
            rescue
            end
            rec[:bike_spr] = bs
            rec[:bike_bmp] = bb
          end
        rescue
        end
      end

      if rec[:state].to_i == 1 || rec[:state].to_i == 4
        sanim = build_surfmon_anim(rec)
        if sanim
          sb = sanim.bitmap
          ss = make_layer_sprite(v, sb, 98)
          begin
            cw = sb.width / 4
            ch = sb.height / 4
            dir = rec[:dir].to_i; dir = 2 if dir <= 0
            pat = rec[:pattern].to_i
            ss.src_rect.set(pat * cw, ((dir - 2) / 2) * ch, cw, ch)
            ss.ox = cw / 2
            ss.oy = ch - 16
          rescue
          end
          rec[:surf_sprite] = ss
          rec[:surf_anim] = sanim
        end
      end

      rec[:bound_map_id] = remote_mid
      rec[:last_outfit_key] = outfit_key
    end
    update_sprite_pos(rec)
  end

  def self.update_sprite_pos(rec)
    s = rec[:sprite]
    return unless s
    locked = ui_locks_peers?
    if locked && rec[:frozen_sx] && rec[:frozen_sy]
      sx = rec[:frozen_sx]
      sy = rec[:frozen_sy]
    else
      tmap = rec[:_tmap] || target_map_for(rec[:map].to_i) || $game_map
      sx, sy = tile_to_screen(rec[:x].to_i, rec[:y].to_i, tmap)
      if locked
        rec[:frozen_sx] = sx
        rec[:frozen_sy] = sy
      else
        rec[:frozen_sx] = nil
        rec[:frozen_sy] = nil
      end
    end
    begin
      dir = rec[:dir].to_i; dir = 2 if dir <= 0
      pat = rec[:pattern].to_i
      action = state_to_action(rec[:state], rec[:cname])
      body_sy = sy
      body_sy = sy + 16 if action == "surf" || action == "dive"
      s.x = sx
      s.y = body_sy
      base_z = calc_z(sy, rec[:y])
      mon_bob = 0
      begin
        mon_bob = ((Graphics.frame_count / 10) % 2) if action == "surf" || action == "dive"
      rescue
      end
      if s.bitmap
        w = (s.bitmap.width >= 4 * FW) ? FW : (s.bitmap.width / 4)
        h = (s.bitmap.height >= 4 * FH) ? FH : (s.bitmap.height / 4)
        w = 1 if w < 1; h = 1 if h < 1
        s.src_rect.set(pat * w, ((dir - 2) / 2) * h, w, h)
        s.ox = w / 2
        s.oy = h
        s.oy -= mon_bob if mon_bob != 0
      end
      s.z = base_z
      apply_tint(s)
      s.visible = true
      if rec[:surf_sprite]
        ss = rec[:surf_sprite]
        ss.x = sx
        ss.y = sy
        if ss.bitmap
          cw = ss.bitmap.width / 4
          ch = ss.bitmap.height / 4
          ss.src_rect.set(pat * cw, ((dir - 2) / 2) * ch, cw, ch)
          ss.ox = cw / 2
          ss.oy = ch - 16
          ss.oy -= mon_bob if mon_bob != 0
        end
        ss.z = base_z - 2
        apply_tint(ss)
        ss.visible = true
      end
      if rec[:bike_spr]
        bs = rec[:bike_spr]
        bs.x = s.x
        bs.y = s.y
        bs.ox = s.ox
        bs.oy = s.oy
        if bs.bitmap
          bs.src_rect.set(pat * FW, ((dir - 2) / 2) * FH, FW, FH)
        end
        bs.z = base_z - 1
        apply_tint(bs)
        bs.visible = true
      end
      ox, oy = offset_for(action, dir, pat)
      extra_y = 0
      extra_y = -2 if (pat % 2 == 1) && action != "surf"
      [[:hair_spr, 1, true],
       [:hat2_spr, 2, false],
       [:hat_spr,  3, false]].each do |key, add, is_hair|
        spr = rec[key]
        next unless spr
        spr.x = s.x + ox
        spr.y = s.y + oy + extra_y
        spr.ox = s.ox
        spr.oy = s.oy
        if spr.bitmap
          fx = pat
          fx = 0 if is_hair && action == "surf"
          if is_hair || spr.bitmap.width >= 4 * FW
            spr.src_rect.set(fx * FW, ((dir - 2) / 2) * FH, FW, FH)
          else
            spr.src_rect.set(0, ((dir - 2) / 2) * FH, [spr.bitmap.width, FW].min, FH)
          end
        end
        spr.z = base_z + add
        apply_tint(spr)
        spr.visible = true
      end
    rescue
    end
  end

  def self.update_labels(rec)
    parent = rec[:sprite]
    return unless parent
    pname = rec[:pname].to_s
    pname = "Joueur" if pname.empty?
    action = rec[:action].to_s
    key = "#{pname}|#{action}"
    v = vp
    return unless v
    if rec[:label_key] != key
      safe_dispose(rec[:label_name])
      safe_dispose(rec[:label_action])
      begin
        rec[:label_name_bmp].dispose if rec[:label_name_bmp]
      rescue
      end
      begin
        rec[:label_action_bmp].dispose if rec[:label_action_bmp]
      rescue
      end
      rec[:label_name] = nil
      rec[:label_action] = nil
      nb = make_bubble(pname, false)
      ns = ::Sprite.new(v)
      ns.bitmap = nb
      ns.ox = nb.width / 2
      ns.oy = nb.height
      rec[:label_name_bmp] = nb
      rec[:label_name] = ns
      if action != ""
        ab = make_bubble(action, true)
        as_ = ::Sprite.new(v)
        as_.bitmap = ab
        as_.ox = ab.width / 2
        as_.oy = ab.height
        rec[:label_action_bmp] = ab
        rec[:label_action] = as_
      end
      rec[:label_key] = key
    end
    begin
      head = parent.y - 56
      name_y = head - 6
      pz = (parent.z rescue 100) + 5
      if rec[:label_name]
        rec[:label_name].x = parent.x
        rec[:label_name].y = name_y
        rec[:label_name].z = pz
        rec[:label_name].visible = parent.visible
      end
      if rec[:label_action]
        rec[:label_action].x = parent.x
        rec[:label_action].y = name_y - 18
        rec[:label_action].z = pz + 1
        rec[:label_action].visible = parent.visible
      end
    rescue
    end
  end

  def self.kill(id)
    rec = @players[id]
    return unless rec
    destroy_player_visuals(rec)
    @players.delete(id)
  end

  def self.tick
    return unless $game_player && $game_map
    now = Time.now.to_f
    begin
      now = System.uptime
    rescue
    end
    return if now - @last < TICK
    @last = now
    begin
      write_me
    rescue
    end
    begin
      return unless $scene.is_a?(Scene_Map)
    rescue
      return
    end
    begin
      read_others
    rescue
    end
    @players.each_key do |id|
      next if id.to_s == (@my_id.to_s rescue "")
      begin
        ensure_all(id)
      rescue
      end
    end
  end

  def self.pre_update
    tick
  end

  def self.post_update
    begin
      return unless $scene.is_a?(Scene_Map)
    rescue
      return
    end
    @players.each_value do |rec|
      next unless rec[:sprite]
      begin
        update_sprite_pos(rec)
      rescue
      end
      begin
        update_labels(rec)
      rescue
      end
    end
  end

  def self.on_spriteset_changed
    @players.each_value { |r| destroy_player_visuals(r) }
  end

  def self.install_hooks!
    return if @hooks_done
    @hooks_done = true
    begin
      if defined?(pbFishingBegin)
        eval("alias _fgl_pbFishingBegin pbFishingBegin unless defined?(_fgl_pbFishingBegin)\ndef pbFishingBegin(*a)\nFGL.instance_variable_set(:@local_fishing,true)\n_fgl_pbFishingBegin(*a)\nend\nalias _fgl_pbFishingEnd pbFishingEnd unless defined?(_fgl_pbFishingEnd)\ndef pbFishingEnd(*a)\nFGL.instance_variable_set(:@local_fishing,false)\n_fgl_pbFishingEnd(*a)\nend")
      end
    rescue
    end
    begin
      if defined?(Graphics)
        meta = (class << Graphics; self; end)
        meta.class_eval do
          unless method_defined?(:_fgl_graphics_update)
            alias_method :_fgl_graphics_update, :update
            def update
              _fgl_graphics_update
              begin
                FGL.tick
              rescue
              end
            end
          end
        end
      end
    rescue
    end
    begin
      if defined?(Scene_Map) && Scene_Map.method_defined?(:update)
        Scene_Map.class_eval do
          unless method_defined?(:_fgl_update)
            alias_method :_fgl_update, :update
            def update
              begin
                FGL.pre_update
              rescue
              end
              _fgl_update
              begin
                FGL.post_update
              rescue
              end
            end
          end
        end
      end
    rescue
    end
    begin
      if defined?(EventHandlers)
        EventHandlers.add(:on_new_spriteset_map, :fgl_net_clear, proc {
          begin
            FGL.on_spriteset_changed
          rescue
          end
        })
      end
    rescue
    end
  end
end

begin
  FGL.install_hooks!
rescue
end