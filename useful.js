// OS GB National Grid (international metres)
var OSNG = new TMgriddata(Airy, 0.9996012717, deg2rad(49), deg2rad(-2), 1, 'm', 400000, -100000, 0, 700000, 0, 1300000);

//TMGrid= OSNG
function TMgriddata(el, sf, lt, ln, ut, un, fe, fn, ea, eb, na, nb) {
    if (el) {
        this.ellip = el;
    } else {
        this.ellip = null;
    }
    if (sf) {
        this.F0 = sf;
    } else {
        this.F0 = 0;
    }
    if (lt) {
        this.Lat0 = lt;
    } else {
        this.Lat0 = 0;
    }
    if (ln) {
        this.Lon0 = ln;
    } else {
        this.Lon0 = 0;
    }
    if (ut) {
        this.unit = ut;
    } else {
        this.unit = 0;
    }
    if (un) {
        this.unitname = un;
    } else {
        this.unitname = '';
    }
    if (fe) {
        this.FE = fe;
    } else {
        this.FE = 0;
    }
    if (fn) {
        this.FN = fn;
    } else {
        this.FN = 0;
    }
    if (ea) {
        this.e_min = ea;
    } else {
        this.e_min = 0;
    }
    if (eb) {
        this.e_max = eb;
    } else {
        this.e_max = 0;
    }
    if (na) {
        this.n_min = na;
    } else {
        this.n_min = 0;
    }
    if (nb) {
        this.n_max = nb;
    } else {
        this.n_max = 0;
    }
}

// Convert OSGB36 to WGS84 - actual GPS Coords
function OSGB362WGS84(g_lat, g_lon) {
    var oldshifty;
    var coointer = Geo2TM(g_lat, g_lon, OSNG);
    var g_east = coointer.eastings;
    var g_north = coointer.northings;
    if (isvalidcoord(g_east, g_north, OSNG)) {
        var shifty = get_osgbshift(g_east, g_north);
        var new_e = g_east - shifty.eastings;
        var new_n = g_north - shifty.northings;
        var complete = false;
        do {
            oldshifty = shifty;
            shifty = get_osgbshift(new_e, new_n);
            if (shifty.eastings == 0 || shifty.northings == 0) {
                complete = true;
            } else {
                if (Math.abs(shifty.eastings - oldshifty.eastings < 0.0001) && Math.abs(shifty.northings - oldshifty.northings < 0.0001)) {
                    complete = true;
                } else {
                    new_e = g_east - shifty.eastings;
                    new_n = g_north - shifty.northings;
                }
            }
        } while (complete == false);
        if (shifty.eastings == 0 || shifty.northings == 0) {
            return new geoextra(Geo2Geo(g_lat, g_lon, OSGB36, WGS84), shifty.extra);
        } else {
            return new geoextra(TM2Geo(new_e, new_n, OSNGgps), shifty.extra);
        }
    } else {
        return new geoextra(Geo2Geo(g_lat, g_lon, OSGB36, WGS84), 'Helmert');
    }
}


// Convert National Grid to Geo Coords (OSGB36)?
function TM2Geo(east, north, TMgrid) {
    var a = TMgrid.ellip.a / TMgrid.unit;
    var b = TMgrid.ellip.b / TMgrid.unit;
    var n = ((a - b) / (a + b));
    var e2 = TMgrid.ellip.e2;
    var A1 = a / (1 + n) * (n * n * (n * n * ((n * n) + 4) + 64) + 256) / 256;
    var h1 = n * (n * (n * (n * (n * (384796 * n - 382725) - 6720) + 932400) - 1612800) + 1209600) / 2419200;
    var h2 = n * n * (n * (n * ((1695744 - 1118711 * n) * n - 1174656) + 258048) + 80640) / 3870720;
    var h3 = n * n * n * (n * (n * (22276 * n - 16929) - 15984) + 12852) / 362880;
    var h4 = n * n * n * n * ((-830251 * n - 158400) * n + 197865) / 7257600;
    var h5 = (453717 - 435388 * n) * n * n * n * n * n / 15966720;
    var h6 = 20648693 * n * n * n * n * n * n / 638668800;
    var M = calc_M(TMgrid.Lat0, TMgrid.Lat0, n, b, TMgrid.F0);
    var E = (north - TMgrid.FN + M) / (A1 * TMgrid.F0);
    var nn = (east - TMgrid.FE) / (A1 * TMgrid.F0);
    var E1i = h1 * Math.sin(2 * E) * cosh(2 * nn);
    var E2i = h2 * Math.sin(4 * E) * cosh(4 * nn);
    var E3i = h3 * Math.sin(6 * E) * cosh(6 * nn);
    var E4i = h4 * Math.sin(8 * E) * cosh(8 * nn);
    var E5i = h5 * Math.sin(10 * E) * cosh(10 * nn);
    var E6i = h6 * Math.sin(12 * E) * cosh(12 * nn);
    var n1i = h1 * Math.cos(2 * E) * sinh(2 * nn);
    var n2i = h2 * Math.cos(4 * E) * sinh(4 * nn);
    var n3i = h3 * Math.cos(6 * E) * sinh(6 * nn);
    var n4i = h4 * Math.cos(8 * E) * sinh(8 * nn);
    var n5i = h5 * Math.cos(10 * E) * sinh(10 * nn);
    var n6i = h6 * Math.cos(12 * E) * sinh(12 * nn);
    var Ei = E - (E1i + E2i + E3i + E4i + E5i + E6i);
    var ni = nn - (n1i + n2i + n3i + n4i + n5i + n6i);
    var B = arcsin(sech(ni) * Math.sin(Ei));
    var l = arcsin(tanh(ni) / Math.cos(B));
    var Q = arsinh(Math.tan(B));
    var Qi = Q + (Math.sqrt(e2) * artanh(Math.sqrt(e2) * tanh(Q)));
    var complete = false;
    do {
        var newv = Q + (Math.sqrt(e2) * artanh(Math.sqrt(e2) * tanh(Qi)));
        if (Math.abs(Qi - newv) < 1e-11) {
            complete = true;
        }
        var Qi = newv;
    } while (complete == false);
    return new geodesic(Math.atan(sinh(Qi)), TMgrid.Lon0 + l);
}