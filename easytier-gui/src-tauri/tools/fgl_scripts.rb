# FGL Scripts helper — Ruby 1.8.7 / RMXP compatible
require 'zlib'

def die(msg)
  STDERR.puts("ERROR: #{msg}")
  exit 1
end

def load_scripts(path)
  die("file not found: #{path}") unless File.exist?(path)
  data = nil
  File.open(path, 'rb') { |f| data = Marshal.load(f) }
  die("Scripts.rxdata is not an Array") unless data.is_a?(Array)
  data
end

def save_scripts(path, data)
  File.open(path, 'wb') { |f| Marshal.dump(data, f) }
end

cmd = ARGV[0]
path = ARGV[1]
out  = ARGV[2] ? ARGV[2] : path
die("usage: fgl_scripts.rb list|inject <Scripts.rxdata> [out]") if cmd.nil? || path.nil?

data = load_scripts(path)

if cmd == 'list'
  i = 0
  data.each do |sc|
    if sc.is_a?(Array) && sc.length >= 2
      puts "#{i}\t#{sc[0]}\t#{sc[1]}"
    else
      puts "#{i}\t?\t?"
    end
    i += 1
  end
  exit 0
end

if cmd == 'inject'
  name = 'FGL_Test'
  # remove previous injection
  data = data.reject { |sc| sc.is_a?(Array) && sc[1].to_s == name }

  src = ""
  src << "# FGL_Test - injection check\n"
  src << "begin\n"
  src << "  f = File.open(\"FGL_INJECT_OK.txt\", \"wb\")\n"
  src << "  f.write(\"FGL injection OK\\n\")\n"
  src << "  f.close\n"
  src << "rescue\n"
  src << "end\n"

  z = Zlib::Deflate.deflate(src)
  max_id = 0
  data.each do |sc|
    if sc.is_a?(Array) && sc[0].is_a?(Integer) && sc[0] > max_id
      max_id = sc[0]
    end
  end
  entry = [max_id + 1, name, z]

  # insert before last script (often Main)
  if data.length > 0
    data.insert(data.length - 1, entry)
  else
    data << entry
  end

  bak = path + ".fglbak"
  unless File.exist?(bak)
    File.open(bak, 'wb') { |o| File.open(path, 'rb') { |i| o.write(i.read) } }
  end

  save_scripts(out, data)
  puts "OK injected #{name} -> #{out}"
  exit 0
end

die("unknown command: #{cmd}")