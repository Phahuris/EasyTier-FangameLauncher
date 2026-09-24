#===============================================================================
# FGL_IPC_Bus — tout Battle/Trade/Remote passe par UDP launcher (pas de FGL_peers)
# Protocole: FGLMSG|PUT|<name>|<payload>
#            FGLMSG|DEL|<name>|
# Binary: payload commence par B64: puis Base64
#===============================================================================
module FGLIPCBus
  @store = {}
  @sock = nil
  @last_err = ""
  @hooks = false

  def self.store; @store; end

  def self.ipc_port
    p = 0
    begin
      if defined?(FGL_RemotePlayer_Test)
        p = FGL_RemotePlayer_Test.send(:ipc_port) rescue 0
      end
    rescue; end
    if p.to_i <= 0
      begin
        p = (ENV["FGL_IPC_PORT"] || "0").to_i
      rescue; p = 0; end
    end
    p.to_i
  end

  def self.ensure_sock
    return @sock if @sock
    begin
      require "socket"
      @sock = UDPSocket.new
      @sock.bind("127.0.0.1", 0)
    rescue => e
      @last_err = "sock:#{e}"
      @sock = nil
    end
    @sock
  end

  def self.ipc_raw(line)
    p = ipc_port
    return if p <= 0
    ensure_sock
    return unless @sock
    begin
      @sock.send(line.to_s, 0, "127.0.0.1", p)
    rescue => e
      @last_err = "send:#{e}"
    end
  end

  def self.encode_bin(data)
    "B64:" + [data.to_s].pack("m0")
  end

  def self.decode_payload(s)
    s = s.to_s
    if s.index("B64:") == 0
      begin
        return s[4, s.length - 4].unpack("m0")[0]
      rescue
        return s
      end
    end
    s
  end

  def self.put(name, data, binary = false)
    name = name.to_s
    raw = data.to_s
    @store[name] = raw
    payload = binary ? encode_bin(raw) : raw
    # | dans payload -> remplacer pour ne pas casser le split (texte simple)
    safe = payload.gsub("|", "\x1f")
    ipc_raw("FGLMSG|PUT|#{name}|#{safe}")
    true
  end

  def self.put_bin(name, obj)
    blob = nil
    begin
      blob = Marshal.dump(obj)
    rescue
      return false
    end
    put(name, blob, true)
  end

  def self.get(name)
    @store[name.to_s]
  end

  def self.get_bin(name)
    raw = get(name)
    return nil if raw.nil? || raw.length < 2
    begin
      return Marshal.load(raw)
    rescue
      return nil
    end
  end

  def self.del(name)
    name = name.to_s
    @store.delete(name)
    ipc_raw("FGLMSG|DEL|#{name}|")
  end

  def self.glob(pattern)
    # pattern style "chal_*" ou "trd_req_*.txt"
    pat = pattern.to_s.gsub(".", "\\.").gsub("*", ".*")
    rx = Regexp.new("^" + pat + "$")
    @store.keys.select { |k| k =~ rx }
  end

  def self.poll
    return if ipc_port <= 0
    ensure_sock
    return unless @sock
    64.times do
      begin
        data = nil
        if @sock.respond_to?(:recvfrom_nonblock)
          begin
            data, _ = @sock.recvfrom_nonblock(65535)
          rescue Exception
            break
          end
        else
          break
        end
        break if data.nil?
        s = data.to_s
        next unless s.index("FGLMSG|") == 0
        parts = s.split("|", 4)
        next if parts.length < 3
        cmd = parts[1].to_s
        name = parts[2].to_s
        if cmd == "PUT" && parts.length >= 4
          payload = parts[3].to_s.gsub("\x1f", "|")
          @store[name] = decode_payload(payload)
        elsif cmd == "DEL"
          @store.delete(name)
        end
      rescue
        break
      end
    end
  end

  def self.install_tick!
    return if @hooks
    @hooks = true
    begin
      meta = (class << Graphics; self; end)
      meta.class_eval do
        unless method_defined?(:_fgl_ipc_bus_update)
          alias_method :_fgl_ipc_bus_update, :update
          define_method(:update) do
            _fgl_ipc_bus_update
            FGLIPCBus.poll rescue nil
          end
        end
      end
    rescue
    end
  end
end

FGLIPCBus.install_tick!